use std::collections::BTreeMap;
use std::sync::Arc;
use std::path::PathBuf;

use async_slot as slot;
use failure::{Error, err_msg};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::Stream;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use serde_json::Value as Json;

use id::Id;
use apply::ApplyData;
use scheduler::Schedule;
use watchdog;


mod compat;
mod wasm;


#[derive(Debug)]
pub enum Request {
    // this one isn't actually send over channel, but we're using it for
    // simplifying interface
    NewSchedule(Arc<Schedule>),
    ForceRerender,
}

#[derive(Debug, Clone)]
pub struct Responder(Arc<Internal>);

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
}

pub fn run(init: ResponderInit) {
    let _guard = watchdog::ExitOnReturn(83);
    let stream = init.schedule_rx.map(Request::NewSchedule)
        .select(init.rx);
    let query_file = init.settings.config_dir
        .join("scheduler/v1/query.wasm");
    let mut responder = Impl::Empty;
    for request in stream.wait() {
        let request = request.expect("stream not closed");
        debug!("Incoming request {:?}", request);
        match request {
            Request::NewSchedule(schedule) => {
                let is_equal = responder.schedule()
                    .map(|x| x.hash == schedule.hash).unwrap_or(false);
                if is_equal {
                    debug!("Same schedule");
                    continue;
                }
                responder = if query_file.exists() {
                    let new = wasm::Responder::new(&schedule,
                        &init.settings, &query_file);
                    match new {
                        Ok(new) => Impl::Wasm(new),
                        Err(e) => {
                            error!("Error initializing query module: {}", e);
                            continue;
                        }
                    }
                } else {
                // TODO(tailhook) check if .wasm exists
                    let new = compat::Responder::new(&schedule, &init.settings);
                    Impl::Compat(new)
                };
                let id: String = thread_rng().sample_iter(&Alphanumeric)
                    .take(24).collect();
                match responder.render_roles(&id) {
                    Ok(data) => {
                        init.apply_tx.swap(ApplyData {
                            id,
                            schedule: schedule.clone(),
                            roles: data,
                        }).ok();
                    }
                    Err(e) => {
                        error!("Can't compute render roles: {}", e);
                    }
                }
            }
            Request::ForceRerender => {
                let schedule = if let Some(s) = responder.schedule() {
                    s.clone()
                } else {
                    // Will render anyway if schedule appears
                    continue;
                };
                let id: String = thread_rng().sample_iter(&Alphanumeric)
                    .take(24).collect();
                match responder.render_roles(&id) {
                    Ok(roles) => {
                        init.apply_tx
                            .swap(ApplyData { id, schedule, roles })
                            .ok();
                    }
                    Err(e) => {
                        error!("Can't compute render roles: {}", e);
                    }
                }
            }
        }
    }
}

impl Impl {
    fn render_roles(&mut self, id: &str)
        -> Result<BTreeMap<String, Json>, Error>
    {
        use self::Impl::*;
        match self {
            Empty => Err(err_msg("no schedule yet")),
            Compat(resp) => resp.render_roles(id),
            Wasm(resp) => resp.render_roles(id),
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
