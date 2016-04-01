use std::str::from_utf8;
use std::net::SocketAddr;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;

use time::{SteadyTime, Duration as Dur};
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
use shared::{Peer, Id};

pub type LeaderFetcher = Spawn<Uniform<Monitor>>;

/// A number of milliseconds after which we consider peer slow, and try
/// to fetch same data from another peer
pub const FETCH_TIME_HINT: i64 = 500;

pub enum Monitor {
    Inactive,
    Copying { addr: SocketAddr, active: Arc<AtomicBool> },
    Prefetching,
}

#[derive(Clone)]
pub enum Connection {
    Follower { active: Arc<AtomicBool> },
    Prefetch { done: bool },
}

pub struct FetchSchedule;

impl Spawner for Monitor {
    type Child = Fsm<Connection, TcpStream>;
    type Seed = (SocketAddr, Connection);
    fn spawn((addr, connection): Self::Seed, scope: &mut Scope<Context>)
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
        return Fsm::new(sock, connection, scope);
    }
}

impl Action for Monitor {
    type Context = Context;
    type Seed = (SocketAddr, Connection);
    fn create(_seed: Self::Seed, _scope: &mut Scope<Self::Context>)
        -> Response<Self, Void>
    {
        unreachable!();
    }
    fn action(self, scope: &mut Scope<Self::Context>)
        -> Response<Self, Self::Seed>
    {
        use self::Monitor::*;
        use scheduler::State::{Unstable, Following, Leading};
        use scheduler::LeaderState::Prefetching as Fetching;
        match *scope.state.scheduler_state() {
            Leading(Fetching(_, ref mutex)) => {
                self.stop_copying();
                let now = SteadyTime::now();
                let time_cut = now - Dur::milliseconds(FETCH_TIME_HINT);
                let lock = &mut *mutex.lock().expect("prefech info locked");
                for (ref hash, ref mut fetching) in lock.fetching.iter_mut() {
                    if lock.all_schedules.contains_key(*hash) {
                        continue;
                    }
                    if fetching.time.map(|x| x < time_cut).unwrap_or(true) {
                        let mut result = None;
                        for id in &fetching.sources {
                            if let Some(addr) = Monitor::get_addr(scope, &id) {
                                fetching.time = Some(now);
                                result = Some((addr, id.clone()));
                            }
                        }
                        if let Some((addr, id)) = result {
                            fetching.sources.remove(&id);
                            return Response::spawn(Prefetching,
                                (addr, Connection::Prefetch { done: false }));
                        } else {
                            warn!("Can't find peer to download {}", hash);
                        }
                    }
                }
                Response::ok(Prefetching)
                    .deadline(scope.now() +
                              // TODO(tailhook) find out exact timeout, maybe
                              Duration::from_millis(FETCH_TIME_HINT as u64/2))
            }
            Leading(_) | Unstable => {
                self.stop_copying();
                Response::ok(Inactive)
            }
            Following(ref id, _) => {
                match Monitor::get_addr(scope, id) {
                    Some(naddr) => {
                        let still_valid = match self {
                            Copying { addr, ref active }
                            if addr == naddr && active.load(SeqCst)
                            => {
                                // TODO(tailhook) notifier.wakeup();
                                true
                            }
                            _ => false,
                        };
                        if still_valid {
                            Response::ok(Inactive)
                        } else {
                            self.stop_copying();
                            let arc = Arc::new(AtomicBool::new(true));
                            return Response::spawn(Copying {
                                addr: naddr,
                                active: arc.clone(),
                            }, (naddr, Connection::Follower { active: arc }));
                        }
                    }
                    None => {
                        self.stop_copying();
                        Response::ok(Inactive)
                    }
                }
            }
        }
    }
}

impl Monitor {
    fn stop_copying(self) {
        use self::Monitor::*;
        match self {
            Inactive|Prefetching => {}
            Copying { active, .. } => {
                active.store(false, SeqCst);
                // TODO(tailhook) notifier.wakeup();
            }
        }
    }
    fn get_addr(scope: &mut Scope<Context>, leader_id: &Id)
        -> Option<SocketAddr>
    {
        if let Some(pair) = scope.state.peers() {
            let peer = pair.1.get(leader_id);
            if let Some(&Peer { addr: Some(addr), .. }) = peer {
                return Some(addr);
            }
        }
        return None;
    }
}

impl Connection {
    fn check_initiate(self, scope: &mut Scope<Context>) -> Task<Connection> {
        use self::Connection::*;
        let fetch = match self {
            Follower { ref active } => {
                scope.state.should_schedule_update() && active.load(SeqCst)
            }
            Prefetch { done: false } => {
                true
            }
            Prefetch { done: true } => {
                return Task::Close
            }
        };
        if fetch {
            Task::Request(self, FetchSchedule)
        } else {
            let timeout = scope.now() + self.idle_timeout(scope);
            Task::Sleep(self, timeout)
        }
    }
}

impl Client for Connection {
    type Seed = Connection;
    type Requester = FetchSchedule;
    fn create(seed: Self::Seed,
        _scope: &mut Scope<<Self::Requester as Requester>::Context>)
        -> Self
    {
        seed
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
                scope.state.fetched_schedule(Schedule {
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
        Spawn::Spawner(Uniform(Monitor::Inactive)),
        scope.notifier(),
    ))
}

impl Drop for Connection {
    fn drop(&mut self) {
        match *self {
            Connection::Follower { ref active } => active.store(false, SeqCst),
            Connection::Prefetch {..} => {},
        }
    }

}
