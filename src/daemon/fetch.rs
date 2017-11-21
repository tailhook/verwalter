use std::mem;
use std::sync::Arc;
use std::process::exit;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use std::net::SocketAddr;

use abstract_ns;
use futures::{Future, Stream, Sink, Async, AsyncSink};
use futures::future::FutureResult;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot;
use rand::{thread_rng, Rng};
use tokio_core::reactor::Timeout;
use tokio_core::net::{TcpStream, TcpStreamNew};
use valuable_futures::{Supply, StateMachine, Async as A, Async as VAsync};

use elect::{self, ScheduleStamp};
use id::Id;
use scheduler::{Schedule, ScheduleId};
use shared::SharedState;
use tk_easyloop::{self, timeout, timeout_at, handle};
use void::{Void, unreachable};
use tk_http::client::{Proto, Codec, Encoder, EncoderDone, Error, RecvMode};
use tk_http::client::{Head, Config};


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
    Replicating(Replica),
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
    Connecting(TcpStreamNew),
    KeepAlive(Proto<TcpStream, ReplicaCodec>),
    Waiting(Proto<TcpStream, ReplicaCodec>, oneshot::Receiver<Arc<Schedule>>),
}

struct ReplicaCodec {
    tx: Option<oneshot::Sender<Arc<Schedule>>>,
}

pub struct Replica {
    leader: Id,
    target: Option<ScheduleId>,
    state: ReplicaConnState,
}

pub struct Context {
    http_config: Arc<Config>,
    shared: SharedState,
    chan: UnboundedReceiver<Message>,
}

pub struct ReplicaContext<'a> {
    http_config: &'a Arc<Config>,
    shared: &'a SharedState,
    leader: &'a Id,
    target: &'a mut Option<ScheduleId>,
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
            Replicating(replica) => match replica.poll(ctx)? {
                A::NotReady(replica) => Replicating(replica),
                A::Ready(v) => unreachable(v),
            },
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
            S::Replicating(Replica { ref leader, .. })
            => P::Replicating { leader: leader.clone() },
            // TODO(tailhook) show failure in public state
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
                (Follower(leader), Replicating(repl)) => {
                    if repl.leader == leader {
                        Replicating(repl)
                    } else {
                        Replicating(Replica {
                            leader: leader,
                            target: None,
                            state: ReplicaConnState::Idle,
                        })
                    }
                }
                (Follower(leader), _) => {
                    Replicating(Replica {
                        leader: leader,
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
                (PeerSchedule(peer, stamp), Replicating(repl)) => {
                    if repl.leader == peer {
                        Replicating(Replica {
                            target: Some(stamp.hash),
                            ..repl
                        })
                    } else {
                        Replicating(repl)
                    }
                }
            }
        }
        return me;
    }
}

fn get_addr(shared: &SharedState, leader: &Id) -> Option<SocketAddr> {
    shared.peers()
        .peers.get(leader)
        .and_then(|peer| peer.get().addr)
}


impl ReplicaConnState {
    fn poll_state(mut self, ctx: ReplicaContext)
        -> ReplicaConnState
    {
        fn reconnect() -> ReplicaConnState {
            Failed(timeout(Duration::from_millis(
                thread_rng().gen_range(100, 300))))
        }
        use self::ReplicaConnState::*;
        loop {
            self = match (self, &mut *ctx.target) {
                (Idle, &mut Some(_)) => {
                    match get_addr(ctx.shared, ctx.leader) {
                        Some(addr) => {
                            Connecting(TcpStream::connect(&addr, &handle()))
                        }
                        None => {
                            warn!("No address of leader: {:?}", ctx.leader);
                            reconnect()
                        }
                    }
                }
                (Idle, &mut None) => return Idle,
                (Failed(mut timeo), _) => {
                    match timeo.poll().expect("infallible") {
                        Async::Ready(()) => Idle,
                        Async::NotReady => return Failed(timeo),
                    }
                }
                (Connecting(mut proto), _) => match proto.poll() {
                    Ok(Async::Ready(sock)) => {
                        KeepAlive(
                            Proto::new(sock, &handle(), &ctx.http_config))
                    }
                    Ok(Async::NotReady) => return Connecting(proto),
                    Err(e) => {
                        debug!("Replica connection error: {:?}", e);
                        reconnect()
                    }
                },
                (KeepAlive(mut proto), &mut Some(_)) => {
                    let (codec, resp) = ReplicaCodec::new();
                    match proto.start_send(codec) {
                        Ok(AsyncSink::NotReady(_)) => return KeepAlive(proto),
                        Ok(AsyncSink::Ready) => Waiting(proto, resp),
                        Err(e) => {
                            debug!("Replica connection error: {:?}", e);
                            reconnect()
                        }
                    }
                }
                (KeepAlive(mut proto), &mut None) => {
                    match proto.poll_complete() {
                        Ok(_) => return KeepAlive(proto),
                        Err(e) => {
                            debug!("Replica connection error: {:?}", e);
                            reconnect()
                        }
                    }
                }
                // TODO(tailhook) receive request
                (Waiting(mut proto, mut chan), targ@&mut Some(_)) => {
                    let pro_poll = proto.poll_complete();
                    let chan_poll = chan.poll();
                    match chan_poll {
                        Ok(Async::Ready(ref sched)) => {
                            if sched.hash == *targ.as_ref().unwrap() {
                                *targ = None;
                                ctx.shared.set_schedule_by_follower(sched);
                            }
                        }
                        _ => {}
                    }
                    match (chan_poll, pro_poll) {
                        (Ok(Async::Ready(_)), Ok(_)) => KeepAlive(proto),
                        (Ok(Async::NotReady), Ok(_)) => {
                            return Waiting(proto, chan)
                        }
                        (_, Err(e)) => {
                            debug!("Replica connection error: {}", e);
                            reconnect()
                        }
                        (Err(e), _) => {
                            debug!("Request dropped unexpectedly: {}", e);
                            // This might be not necessary but ocasionally
                            // reconnecting is fine
                            reconnect()
                        }
                    }
                }
                (Waiting(mut proto, chan), targ@&mut None) => {
                    unreachable!();
                }
            }
        }
    }
}

impl StateMachine for Replica {
    type Supply = Context;
    type Item = Void;
    type Error = Void;
    fn poll(self, ctx: &mut Context) -> Result<VAsync<Void, Self>, Void> {
        let Replica { mut target, state, leader } = self;
        let state = state.poll_state(ReplicaContext {
            leader: &leader,
            http_config: &ctx.http_config,
            shared: &ctx.shared,
            target: &mut target,
        });
        return Ok(VAsync::NotReady(Replica { target, state, leader }));
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
            http_config: Config::new()
                .inflight_request_limit(1)
                .keep_alive_timeout(Duration::new(300, 0))
                .max_request_timeout(Duration::new(3, 0))
                .done(),
        }, Fetch::Unstable)
        .map(|v| unreachable(v)).map_err(|e| unreachable(e)));
    Ok(())
}

impl ReplicaCodec {
    fn new() -> (ReplicaCodec, oneshot::Receiver<Arc<Schedule>>) {
        let (tx, rx) = oneshot::channel();
        (ReplicaCodec { tx: Some(tx) }, rx)
    }
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
