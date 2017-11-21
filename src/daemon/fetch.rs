use std::mem;
use std::sync::Arc;
use std::process::exit;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::net::SocketAddr;

use abstract_ns;
use futures::{Future, Stream, Async};
use futures::future::FutureResult;
use futures::sync::mpsc::UnboundedReceiver;
use tokio_core::reactor::Timeout;
use tokio_core::net::TcpStream;
use valuable_futures::{Supply, StateMachine, Async as A, Async as VAsync};

use elect::{self, ScheduleStamp};
use id::Id;
use scheduler::{Schedule, ScheduleId};
use shared::SharedState;
use tk_easyloop::{self, timeout, timeout_at};
use void::{Void, unreachable};
use tk_http::client::{Proto, Codec, Encoder, EncoderDone, Error, RecvMode};
use tk_http::client::{Head};


const PREFETCH_MIN: u64 = elect::settings::HEARTBEAT_INTERVAL * 3/2;
const PREFETCH_MAX: u64 = PREFETCH_MIN + 60_000;


pub enum Message {
    Leader,
    Follower(Id),
    Election,
    PeerSchedule(Id, ScheduleStamp),
}

enum Fetch {
    Unstable,
    StableLeader,
    Prefetching(Prefetch),
    Replicating(Id, Replica),
    NoLeaderAddress(Id),
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(tag="state")]
pub enum PublicState {
    Unstable,
    StableLeader,
    Prefetching(PrefetchState),
    FollowerWaiting { leader: Id },
    Replicating { leader: Id },
    Following { leader: Id },
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PrefetchState {
    Graceful,
    Fetching,
}

pub struct Prefetch {
    start: Instant,
    state: PrefetchState,
    timeout: Timeout,
    schedules: HashMap<ScheduleId, Arc<Schedule>>,
    waiting: HashMap<ScheduleId, HashSet<Id>>,
}

enum ReplicaConnState {
    Idle,
    Failed(Timeout),
    KeepAlive(Proto<TcpStream, ReplicaCodec>),
    Waiting(Proto<TcpStream, ReplicaCodec>),
}

struct ReplicaCodec {
}

pub struct Replica {
    target: Option<ScheduleId>,
    state: ReplicaConnState,
}

pub struct Context {
    shared: SharedState,
    chan: UnboundedReceiver<Message>,
}

impl StateMachine for Fetch {
    type Supply = Context;
    type Item = Void;
    type Error = Void;
    fn poll(mut self, ctx: &mut Context)
        -> Result<VAsync<Void, Fetch>, Void>
    {
        use self::Fetch::*;
        self = self.poll_messages(ctx);
        self = match self {
            Unstable => Unstable,
            StableLeader => StableLeader,
            Prefetching(pre) => match pre.poll(ctx)? {
                A::NotReady(pre) => Prefetching(pre),
                A::Ready(prefetch_data) => {
                    // TODO(tailhook) send parent schedules
                    ctx.shared.set_parents(prefetch_data);
                    StableLeader
                }
            },
            Replicating(id, replica) => match replica.poll(ctx)? {
                A::NotReady(replica) => Replicating(id, replica),
                A::Ready(v) => unreachable(v),
            },
            NoLeaderAddress(id) => NoLeaderAddress(id),
        };
        let state = self.public_state();
        if *ctx.shared.fetch_state.get() != state {
            ctx.shared.fetch_state.set(Arc::new(state));
        }
        Ok(A::NotReady(self))
    }
}

impl Fetch {
    fn public_state(&self) -> PublicState {
        use self::Fetch as S;
        use self::PublicState as P;
        match *self {
            S::Unstable => P::Unstable,
            S::StableLeader => P::StableLeader,
            S::Prefetching(Prefetch { state, .. } ) => P::Prefetching(state),
            // TODO(tailhook) unpack replication state
            S::Replicating(ref id, ..)
            => P::Replicating { leader: id.clone() },
            // TODO(tailhook) show failure in public state
            S::NoLeaderAddress(ref id, ..)
            => P::Replicating { leader: id.clone() },
        }
    }
    fn poll_messages(self, ctx: &mut Context) -> Self {
        use self::Message::*;
        use self::Fetch::*;
        let mut me = self;
        while let Async::Ready(value) = ctx.chan.poll().expect("infallible") {
            let msg = if let Some(x) = value { x }
                else {
                    error!("Premature exit of fetch channel");
                    exit(82);
                };
            me = match (msg, me) {
                (Leader, StableLeader) => StableLeader,
                (Leader, m @ Prefetching(..)) => m,
                (Leader, _) => {
                    // TODO(tailhook) drop schedule
                    let mut schedules = HashMap::new();
                    if let Some(sch) = ctx.shared.schedule() {
                        schedules.insert(sch.hash.clone(), sch);
                    }
                    let dline = Instant::now() +
                        Duration::from_millis(PREFETCH_MIN);
                    Prefetching(Prefetch {
                        start: Instant::now(),
                        state: PrefetchState::Graceful,
                        timeout: timeout_at(dline),
                        schedules,
                        waiting: HashMap::new(),
                    })
                }
                (Follower(d), Replicating(s, r)) => {
                    if s == d {
                        Replicating(s, r)
                    } else {
                        Replicating(d, Replica {
                            target: None,
                            state: ReplicaConnState::Idle,
                        })
                    }
                }
                (Follower(id), _) => {
                    Replicating(id, Replica {
                        target: None,
                        state: ReplicaConnState::Idle,
                    })
                }
                (Election, Unstable) => Unstable,
                (Election, _) => {
                    // TODO(tailhook) drop schedule
                    Unstable
                }
                (PeerSchedule(_, _), Unstable) => Unstable, // ignore
                (PeerSchedule(_, _), StableLeader) => StableLeader, // ignore
                (PeerSchedule(ref id, ref stamp), Prefetching(ref mut pre))
                => {
                    unimplemented!()
                    // pre.report(id, stamp)
                }
                (PeerSchedule(d, stamp), Replicating(s, rep)) => {
                    if s == d {
                        Replicating(s, Replica {
                            target: Some(stamp.hash),
                            ..rep
                        })
                    } else {
                        Replicating(d, Replica {
                            target: Some(stamp.hash),
                            state: ReplicaConnState::Idle,
                        })
                    }
                }
                (PeerSchedule(id, stamp), NoLeaderAddress(..)) => {
                    Replicating(id, Replica {
                        target: Some(stamp.hash),
                        state: ReplicaConnState::Idle,
                    })
                }
            }
        }
        return me;
    }
}

impl StateMachine for Replica {
    type Supply = Context;
    type Item = Void;
    type Error = Void;
    fn poll(self, ctx: &mut Context) -> Result<VAsync<Void, Self>, Void> {
        /*
        match self.state {
            Idle => Idle,
            Failed(mut timeo) => {}
            KeepAlive(Proto<TcpStream, ReplicaCodec>),
            Waiting(Proto<TcpStream, ReplicaCodec>),
        }
        */
        unimplemented!();
    }
}

impl Prefetch {
    fn get_state(&mut self) -> Vec<Arc<Schedule>> {
        self.schedules.drain().map(|(_, v)| v).collect()
    }
}

impl StateMachine for Prefetch {
    type Supply = Context;
    type Item = Vec<Arc<Schedule>>;
    type Error = Void;
    fn poll(self, ctx: &mut Context) -> Result<VAsync<Self::Item, Self>, Void>
    {
        /*
        if self.state == PrefetchState::Fetching && self.waiting.len() == 0 {
            return Ok(Async::Ready(self.get_state()));
        }
        match self.timeout.poll().expect("timeout never fails") {
            Async::Ready(()) if self.state == PrefetchState::Graceful => {
                self.state = PrefetchState::Fetching;
                self.timeout = timeout_at(self.start +
                    Duration::from_millis(PREFETCH_MAX));
                match self.timeout.poll().expect("timeout never fails") {
                    Async::Ready(()) => {
                        return Ok(Async::Ready(self.get_state()));
                    }
                    Async::NotReady => {}
                }
            }
            Async::Ready(()) => {
                return Ok(Async::Ready(self.get_state()));
            }
            Async::NotReady => {}
        }
        Ok(Async::NotReady)
        */
        unimplemented!();
    }
}

pub fn spawn_fetcher(ns: &abstract_ns::Router, state: &SharedState,
    chan: UnboundedReceiver<Message>)
    -> Result<(), Box<::std::error::Error>>
{
    tk_easyloop::spawn(Supply::new(Context {
            shared: state.clone(),
            chan: chan,
        }, Fetch::Unstable)
        .map(|v| unreachable(v)).map_err(|e| unreachable(e)));
    Ok(())
}

impl Codec<TcpStream> for ReplicaCodec {
    type Future = FutureResult<EncoderDone<TcpStream>, Error>;
    fn start_write(&mut self, e: Encoder<TcpStream>) -> Self::Future {
        unimplemented!();
    }
    fn headers_received(&mut self, headers: &Head) -> Result<RecvMode, Error> {
        unimplemented!();
    }
    fn data_received(
        &mut self,
        data: &[u8],
        end: bool
    ) -> Result<Async<usize>, Error> {
        assert!(end);
        unimplemented!();
    }
}
