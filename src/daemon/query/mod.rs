use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::path::PathBuf;
use std::time::Instant;

use async_slot as slot;
use failure::{Error, err_msg};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::sync::oneshot;
use futures::{Future, Stream};
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use serde_json::Value as Json;
use void::Void;

use id::Id;
use apply::ApplyData;
use scheduler::Schedule;
use shared::SharedState;
use watchdog;


mod compat;
mod wasm;


#[derive(Debug)]
pub enum Request {
    // this one isn't actually send over channel, but we're using it for
    // simplifying interface
    NewSchedule(Arc<Schedule>),
    RolesData(oneshot::Sender<Result<RolesResult, Error>>),
    Query(QueryData, oneshot::Sender<Result<Json, Error>>),
    ForceRerender,
}

#[derive(Debug, Clone)]
pub struct Responder(Arc<Internal>);

#[derive(Debug, Clone, Serialize)]
pub struct QueryData {
    pub path: String,
    pub body: Json,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RolesResult {
    to_render: BTreeMap<String, Json>,
    all_roles: HashSet<String>,
    // TODO(tailhook)
    //#[serde(with="::serde_millis")]
    //next_render: Option<SystemTime>,
}

#[derive(Debug)]
struct Internal {
    schedule_tx: slot::Sender<Arc<Schedule>>,
    tx: UnboundedSender<Request>,
}

pub struct ResponderInit {
    rx: UnboundedReceiver<Request>,
    // coalesce subsequent schedules, prioritize over requests
    schedule_rx: slot::Receiver<Arc<Schedule>>,
    apply_tx: slot::Sender<ApplyData>,
    settings: Settings,
}

pub struct Settings {
    pub id: Id,
    pub hostname: String,
    pub config_dir: PathBuf,
}

enum Impl {
    Empty,
    Compat(self::compat::Responder),
    Wasm(self::wasm::Responder),
}

impl Responder {
    pub fn new(apply_tx: slot::Sender<ApplyData>, settings: Settings)
        -> (Responder, ResponderInit)
    {
        let (tx, rx) = unbounded();
        let (schedule_tx, schedule_rx) = slot::channel();
        let resp = Responder(Arc::new(Internal {
            tx,
            schedule_tx,
        }));
        let init = ResponderInit {
            rx,
            apply_tx,
            schedule_rx,
            settings,
        };
        return (resp, init);
    }
    pub fn new_schedule(&self, sched: Arc<Schedule>) {
        self.0.schedule_tx.swap(sched)
            .expect("schedule channel works");
    }
    pub fn force_rerender(&self) {
        self.0.tx.unbounded_send(Request::ForceRerender)
            .expect("responder channel works");
    }
    pub fn get_roles_data(&self)
        -> impl Future<Item=Result<RolesResult, Error>, Error=Void>+Send
    {
        let (tx, rx) = oneshot::channel();
        self.0.tx.unbounded_send(Request::RolesData(tx))
            .expect("responder channel works");
        return rx.map_err(|e| {
            error!("responder lost the channel: {}", e);
            unreachable!("responder lost the channel");
        });
    }
    pub fn query(&self, query: QueryData)
        -> impl Future<Item=Result<Json, Error>, Error=Void>+Send
    {
        let (tx, rx) = oneshot::channel();
        self.0.tx.unbounded_send(Request::Query(query, tx))
            .expect("responder channel works");
        return rx.map_err(|e| {
            error!("responder lost the channel: {}", e);
            unreachable!("responder lost the channel");
        });
    }
}

pub fn run(init: ResponderInit, shared: SharedState) {
    let _guard = watchdog::ExitOnReturn(83);
    let stream = init.schedule_rx.map(Request::NewSchedule)
        .select(init.rx);
    let query_file = init.settings.config_dir
        .join("scheduler/v1/query.wasm");
    let mut responder = Impl::Empty;
    for request in stream.wait() {
        let request = request.expect("stream not closed");
        match request {
            Request::NewSchedule(schedule) => {
                let prev = responder.schedule();
                let is_equal = prev.as_ref()
                    .map(|x| x.hash == schedule.hash).unwrap_or(false);
                debug!("Incoming request NewSchedule: {} {}",
                    schedule.hash, if is_equal { "old" } else { "new" });
                if is_equal {
                    continue;
                }
                responder = if query_file.exists() {
                    let new = wasm::Responder::new(&schedule,
                        &init.settings, &query_file);
                    match new {
                        Ok(new) => {
                            debug!("Initialized wasm query engine");
                            Impl::Wasm(new)
                        }
                        Err(e) => {
                            error!("Error initializing query module: {}", e);
                            continue;
                        }
                    }
                } else {
                    let new = compat::Responder::new(&schedule, &init.settings);
                    debug!("Initialized compatibility query engine");
                    Impl::Compat(new)
                };
                let id: String = thread_rng().sample_iter(&Alphanumeric)
                    .take(24).collect();
                let prev_ref = prev.as_ref().map(|x| &**x);
                let start = Instant::now();
                match responder.render_roles(&id, prev_ref) {
                    Ok(data) => {
                        let elapsed = start.elapsed();
                        shared.update_role_list(&data.all_roles);
                        info!("Roles data: {} roles to render, \
                            {} total, in {:?}",
                            data.to_render.len(), data.all_roles.len(),
                            elapsed);
                        init.apply_tx.swap(ApplyData {
                            id,
                            schedule: schedule.clone(),
                            roles: data.to_render,
                        }).ok();
                    }
                    Err(e) => {
                        error!("Can't compute render roles: {}", e);
                    }
                }
            }
            Request::ForceRerender => {
                debug!("Incoming request ForceRerender");
                let schedule = if let Some(s) = responder.schedule() {
                    s.clone()
                } else {
                    // Will render anyway if schedule appears
                    continue;
                };
                let id: String = thread_rng().sample_iter(&Alphanumeric)
                    .take(24).collect();
                match responder.render_roles(&id, None) {
                    Ok(data) => {
                        shared.update_role_list(&data.all_roles);
                        init.apply_tx
                            .swap(ApplyData {
                                id,
                                schedule,
                                roles: data.to_render,
                            })
                            .ok();
                    }
                    Err(e) => {
                        error!("Can't compute render roles: {}", e);
                    }
                }
            }
            Request::RolesData(chan) => {
                debug!("Incoming request RolesData");
                // maybe store previous id?
                let id: String = thread_rng().sample_iter(&Alphanumeric)
                    .take(24).collect();
                chan.send(responder.render_roles(&id, None))
                    .map_err(|_| debug!("no receiver for RolesData"))
                    .ok();
            }
            Request::Query(query_data, chan) => {
                debug!("Incoming request Query");
                chan.send(responder.query(query_data))
                    .map_err(|_| debug!("no receiver for RolesData"))
                    .ok();
            }
        }
    }
}

impl Impl {
    fn render_roles(&mut self, id: &str, prev: Option<&Schedule>)
        -> Result<RolesResult, Error>
    {
        use self::Impl::*;
        match self {
            Empty => Err(err_msg("no schedule yet")),
            Compat(resp) => resp.render_roles(id, prev),
            Wasm(resp) => resp.render_roles(id, prev),
        }
    }
    fn query(&mut self, query: QueryData)
        -> Result<Json, Error>
    {
        use self::Impl::*;
        match self {
            Empty => Err(err_msg("no schedule yet")),
            Compat(resp) => resp.query(query),
            Wasm(resp) => resp.query(query),
        }
    }
    fn schedule(&self) -> Option<Arc<Schedule>> {
        use self::Impl::*;
        match self {
            Empty => None,
            Compat(resp) => Some(resp.schedule()),
            Wasm(resp) => Some(resp.schedule()),
        }
    }
}
