use std::str::from_utf8;
use std::net::SocketAddr;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;

use rotor::{Scope, Response, Time, GenericScope, Notifier, Void};
use rotor::mio::tcp::TcpStream;
use rotor_tools::compose::{Spawn, Spawner};
use rotor_tools::uniform::{Uniform, Action};
use rotor_http;
use rotor_http::client::{Fsm, Client, Requester, Task, Version};
use rotor_http::client::{Request, RecvMode, ResponseError, Head};
use rustc_serialize::json::Json;

use net::Context;
use hash::hash;
use scheduler::{Schedule};
use shared::{Peer};

pub type LeaderFetcher = Spawn<Uniform<Monitor>>;

pub struct Monitor(Option<(SocketAddr, Arc<AtomicBool>)>);
pub struct Connection(Arc<AtomicBool>);
pub struct FetchSchedule;

impl Spawner for Monitor {
    type Child = Fsm<Connection, TcpStream>;
    type Seed = (SocketAddr, Arc<AtomicBool>);
    fn spawn((addr, arc): Self::Seed, scope: &mut Scope<Context>)
        -> Response<Self::Child, Void>
    {
        let sock = match TcpStream::connect(&addr) {
            Ok(sock) => sock,
            Err(e) => {
                error!("Error connecting to leader: {}", e);
                // TODO(tailhook) reconnect now?
                return Response::done();
            }
        };
        return Fsm::new(sock, arc, scope);
    }
}

impl Action for Monitor {
    type Context = Context;
    type Seed = (SocketAddr, Arc<AtomicBool>);
    fn create(_seed: Self::Seed, _scope: &mut Scope<Self::Context>)
        -> Response<Self, Void>
    {
        unreachable!();
    }
    fn action(mut self, scope: &mut Scope<Self::Context>)
        -> Response<Self, Self::Seed>
    {
        let el = scope.state.election();
        if let Some(ref leader_id) = el.leader {
            if let Some(pair) = scope.state.peers() {
                let peer = pair.1.get(leader_id);
                if let Some(&Peer { addr: Some(addr), .. }) = peer {
                    let changed = (self.0).as_ref().map(|x| x.0 != addr ||
                        x.1.load(SeqCst) == false).unwrap_or(true);
                    if changed {
                        debug!("New fetch from {:?}", leader_id);
                        self.0.map(|(_, x)| x.store(false, SeqCst));
                        let arc = Arc::new(AtomicBool::new(true));
                        self.0 = Some((addr, arc.clone()));
                        return Response::spawn(self, (addr, arc));
                    }
                } else {
                    error!("There is a leader ({}) but no address", leader_id);
                }
            } else {
                error!("There is a leader ({}) but no peers", leader_id);
            }
        }
        Response::ok(self)
    }
}

impl Connection {
    fn is_active(&self) -> bool {
        self.0.load(SeqCst)
    }
    fn check_initiate(self, scope: &mut Scope<Context>) -> Task<Connection> {
        if scope.state.should_schedule_update() && self.is_active() {
            Task::Request(self, FetchSchedule)
        } else {
            let timeout = scope.now() + self.idle_timeout(scope);
            Task::Sleep(self, timeout)
        }
    }
}

impl Client for Connection {
    type Seed = Arc<AtomicBool>;
    type Requester = FetchSchedule;
    fn create(seed: Self::Seed,
        _scope: &mut Scope<<Self::Requester as Requester>::Context>)
        -> Self
    {
        Connection(seed)
    }
    fn connection_idle(self, _connection: &rotor_http::client::Connection,
        scope: &mut Scope<Context>)
        -> Task<Self>
    {
        self.check_initiate(scope)
    }
    fn wakeup(self, connection: &rotor_http::client::Connection,
        scope: &mut Scope<Context>)
        -> Task<Self>
    {
        if connection.is_idle() {
            self.check_initiate(scope)
        } else {
            let timeout = scope.now() + self.idle_timeout(scope);
            Task::Sleep(self, timeout)
        }
    }
    fn timeout(self,
        _connection: &rotor_http::client::Connection,
        _scope: &mut Scope<<Self::Requester as Requester>::Context>)
        -> Task<Self>
    {
        Task::Close
    }
}

impl Requester for FetchSchedule {
    type Context = Context;
    fn prepare_request(self, req: &mut Request,
        _scope: &mut Scope<Self::Context>) -> Option<Self>
    {
        req.start("GET", "/v1/schedule", Version::Http10);
        req.done_headers().unwrap();
        req.done();
        Some(self)
    }
    fn headers_received(self, head: Head, _request: &mut Request,
        scope: &mut Scope<Self::Context>)
        -> Option<(Self, RecvMode, Time)>
    {
        if head.code == 200 {
            Some((self, RecvMode::Buffered(1_048_576),
                scope.now() + Duration::new(10, 0)))
        } else {
            error!("Error fetching schedule, status code: {}", head.code);
            None
        }
    }
    fn response_received(self, data: &[u8], _request: &mut Request,
        scope: &mut Scope<Self::Context>)
    {
        let s = match from_utf8(data) {
            Ok(s) => s,
            Err(e) => {
                error!("Error decoding utf8 for schedule: {}", e);
                debug!("Undecodable data: {:?}", data);
                return;
            }
        };
        let mut j = match Json::from_str(s) {
            Ok(Json::Object(ob)) => ob,
            Ok(v) => {
                error!("Wrong data type for schedule, data: {:?}", v);
                return;
            }
            Err(e) => {
                error!("Error decoding json for schedule: {}", e);
                debug!("Undecodable data: {:?}", s);
                return;
            }
        };
        let hashvalue = j.remove("hash");
        let origin = j.remove("origin")
            .and_then(|x| x.as_string().and_then(|x| x.parse().ok()));
        let timestamp = j.remove("timestamp").and_then(|x| x.as_u64());
        let data = j.remove("data");
        match (hashvalue, timestamp, data, origin) {
            (Some(Json::String(h)), Some(t), Some(d), Some(o)) => {
                let hash = hash(d.to_string());
                if hash != h {
                    error!("Invalid hash {:?} data {}", h, d);
                    return;
                }
                debug!("Fetched schedule {:?}", hash);
                scope.state.set_schedule_if_matches(Schedule {
                    timestamp: t,
                    hash: h.to_string(),
                    data: d,
                    origin: o,
                });
            }
            (hash, tstamp, data, origin) => {
                error!("Wrong data in the schedule, \
                    values: {:?} -- {:?} -- {:?} -- {:?}",
                    hash, tstamp, data, origin);
                return;
            }
        }
    }
    fn bad_response(self, error: &ResponseError,
        _scope: &mut Scope<Self::Context>)
    {
        error!("Can't fetch config from the leader: {}", error);
    }
    fn response_chunk(self, _chunk: &[u8], _request: &mut Request,
        _scope: &mut Scope<Self::Context>)
        -> Option<Self>
    {
        unreachable!();
    }
    fn response_end(self, _request: &mut Request,
        _scope: &mut Scope<Self::Context>)
    {
        unreachable!();
    }
    fn timeout(self, _request: &mut Request, _scope: &mut Scope<Self::Context>)
        -> Option<(Self, Time)>
    {
        unimplemented!();
    }
    fn wakeup(self, _request: &mut Request, _scope: &mut Scope<Self::Context>)
        -> Option<Self>
    {
        unimplemented!();
    }
}

pub fn create<S: GenericScope>(scope: &mut S)
    -> Response<(LeaderFetcher, Notifier), Void>
{
    Response::ok((
        Spawn::Spawner(Uniform(Monitor(None))),
        scope.notifier(),
    ))
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.0.store(false, SeqCst);
    }
}
