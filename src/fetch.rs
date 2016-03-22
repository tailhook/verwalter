use std::str::from_utf8;
use std::time::Duration;

use rotor::{Scope, Response, Time, GenericScope, Notifier};
use rotor::void::{Void, unreachable};
use rotor::mio::tcp::TcpStream;
use rotor_tools::compose::Spawn;
use rotor_tools::uniform::{Uniform, Action};
use rotor_http;
use rotor_http::client::{Fsm, Client, Requester, Task, Version};
use rotor_http::client::{Request, RecvMode, ResponseError, Head};
use rustc_serialize::json::Json;

use net::Context;
use hash::hash;
use shared::Schedule;

pub type LeaderFetcher = Spawn<Uniform<Monitor>, Fsm<Connection, TcpStream>>;

pub struct Monitor;
pub struct Connection;
pub struct FetchSchedule;

impl Action for Monitor {
    type Context = Context;
    type Seed = Void;
    fn create(seed: Self::Seed, _scope: &mut Scope<Self::Context>)
        -> Response<Self, Void>
    {
        unreachable(seed);
    }
    fn action(self, scope: &mut Scope<Self::Context>)
        -> Response<Self, Self::Seed>
    {
        let el = scope.state.election();
        if let Some(ref leader_id) = el.leader {
            println!("Fetch from {:?}", leader_id);
        }
        Response::ok(self)
    }
}

impl Connection {
    fn check_initiate(self) -> Task<Connection> {
        unimplemented!();
    }
}

impl Client for Connection {
    type Seed = ();
    type Requester = FetchSchedule;
    fn create(_seed: Self::Seed,
        _scope: &mut Scope<<Self::Requester as Requester>::Context>)
        -> Self
    {
        Connection
    }
    fn connection_idle(self, _connection: &rotor_http::client::Connection,
        _scope: &mut Scope<Context>)
        -> Task<Self>
    {
        self.check_initiate()
    }
    fn wakeup(self, connection: &rotor_http::client::Connection,
        scope: &mut Scope<Context>)
        -> Task<Self>
    {
        if connection.is_idle() {
            self.check_initiate()
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
        let timestamp = j.remove("timestamp").and_then(|x| x.as_u64());
        let data = j.remove("data");
        match (hashvalue, timestamp, data) {
            (Some(Json::String(h)), Some(t), Some(d)) => {
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
                    origin: false,
                });
            }
            (hash, tstamp, data) => {
                error!("Wrong data in the schedule, \
                    values: {:?} -- {:?} -- {:?}",
                    hash, tstamp, data);
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
        Spawn::Spawner(Uniform(Monitor)),
        scope.notifier(),
    ))
}
