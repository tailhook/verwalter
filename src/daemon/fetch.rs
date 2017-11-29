use std::io;
use std::cmp::max;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{Future, Stream, Sink, Async, AsyncSink};
use futures::future::{FutureResult, ok};
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot;
use rand::{thread_rng, Rng, sample};
use tokio_core::reactor::Timeout;
use tokio_core::net::{TcpStream, TcpStreamNew};
use valuable_futures::{Supply, StateMachine, Async as A, Async as VAsync};

use elect::{ScheduleStamp};
use id::Id;
use scheduler::{self, Schedule, ScheduleId};
use serde_json;
use shared::SharedState;
use tk_easyloop::{self, timeout, timeout_at, handle};
use void::{Void, unreachable};
use tk_http::client::{Proto, Codec, Encoder, EncoderDone, Error, RecvMode};
use tk_http::client::{Head, Config};
use tk_http::{Version, Status};
use failures::Blacklist;


/// Minimum time to spend on prefetching
///
/// Absolute minimum for this value is about 2.5 heartbeat intervals (now it's
/// about 1.2 sec, but we still want some spare time, because it's important
/// to get all the schedules.
const PREFETCH_MIN: u64 = 15_000;
// elect::settings::HEARTBEAT_INTERVAL * 5/2;
/// A deadline for prefetching
///
/// If some hosts are unavailable and we know that they had different
/// schedules, we wait this long, and then bail out.
const PREFETCH_MAX: u64 = PREFETCH_MIN + 60_000;
/// Find next host if some request hangs for more than this amount of ms
/// Note: we don't cancel request, just run another in parallel
const PREFETCH_OLD: u64 = 1000;
/// This is randomized 0.5 - 1.5 of the value for randomized reconnects
const PREFETCH_BLACKLIST: u64 = 200;


type ScheduleRecv = oneshot::Receiver<Result<Arc<Schedule>, ReplicaError>>;


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
    Replicating { leader: Id, schedule: Option<ScheduleId> },
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
    blacklist: Blacklist,
    fetching: VecDeque<(FetchContext, FetchState)>,
}

enum ReplicaConnState {
    Idle,
    Failed(Timeout),
    Connecting(TcpStreamNew),
    KeepAlive(Proto<TcpStream, ReplicaCodec>),
    Waiting(Proto<TcpStream, ReplicaCodec>, ScheduleRecv),
}

struct FetchContext {
    started: Instant,
    schedule: ScheduleId,
    addr: SocketAddr,
    http_config: Arc<Config>,
}

enum FetchState {
    Connecting(TcpStreamNew),
    WaitRequest(Proto<TcpStream, ReplicaCodec>),
    WaitResponse(Proto<TcpStream, ReplicaCodec>, ScheduleRecv),
}

#[derive(Fail, Debug)]
enum ReplicaError {
    #[fail(display="invalid status {:?}", _0)]
    InvalidStatus(Option<Status>),
    #[fail(display="serde error: {:?}", _0)]
    SerdeError(serde_json::Error),
    #[fail(display="could not decode schedule: {}", _0)]
    ScheduleError(String),
    #[fail(display="http error: {}", _0)]
    Http(Error),
    #[fail(display="IO error: {}", _0)]
    Io(io::Error),
    #[fail(display="oneshot was canceled")]
    OneshotCancel,
}

struct ReplicaCodec {
    tx: Option<oneshot::Sender<Result<Arc<Schedule>, ReplicaError>>>,
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
            S::Replicating(Replica { ref leader, ref target, .. })
            => P::Replicating { leader: leader.clone(),
                                schedule: target.clone() },
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
                    ctx.shared.reset_stable_schedule();
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
                        blacklist: Blacklist::new(&handle()),
                        fetching: VecDeque::new(),
                    })
                }
                (Follower(leader), Replicating(repl)) => {
                    if repl.leader == leader {
                        Replicating(repl)
                    } else {
                        ctx.shared.reset_stable_schedule();
                        Replicating(Replica {
                            leader: leader,
                            target: None,
                            state: ReplicaConnState::Idle,
                        })
                    }
                }
                (Follower(leader), _) => {
                    ctx.shared.reset_stable_schedule();
                    Replicating(Replica {
                        leader: leader,
                        target: None,
                        state: ReplicaConnState::Idle,
                    })
                }
                (Election, Unstable) => Unstable,
                (Election, _) => {
                    ctx.shared.reset_stable_schedule();
                    Unstable
                }
                (PeerSchedule(_, _), Unstable) => Unstable, // ignore
                (PeerSchedule(_, _), StableLeader) => StableLeader, // ignore
                (PeerSchedule(id, stamp), Prefetching(mut pre))
                => {
                    pre.report(id, stamp.hash);
                    Prefetching(pre)
                }
                (PeerSchedule(peer, stamp), Replicating(repl)) => {
                    if repl.leader == peer {
                        if repl.target == None &&
                            ctx.shared.is_current(&stamp.hash)
                        {
                            Replicating(repl)
                        } else {
                            ctx.shared.reset_stable_schedule();
                            Replicating(Replica {
                                target: Some(stamp.hash),
                                ..repl
                            })
                        }
                    } else {
                        Replicating(repl)
                    }
                }
            }
        }
        return me;
    }
}

fn get_addr(shared: &SharedState, peer_id: &Id) -> Option<SocketAddr> {
    shared.peers()
        .peers.get(peer_id)
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
                (Waiting(mut proto, mut chan), targ@&mut Some(_)) => {
                    let pro_poll = proto.poll_complete();
                    let chan_poll = chan.poll();
                    match chan_poll {
                        Ok(Async::Ready(Ok(ref sched))) => {
                            if sched.hash == *targ.as_ref().unwrap() {
                                *targ = None;
                                ctx.shared.set_schedule_by_follower(sched);
                            }
                        }
                        _ => {}
                    }
                    match (chan_poll, pro_poll) {
                        (Ok(Async::Ready(Ok(_))), Ok(_)) => KeepAlive(proto),
                        (Ok(Async::Ready(Err(e))), Ok(_)) => {
                            info!("Replica request error: {}. \
                                   Will reconnect...", e);
                            reconnect()
                        }
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
                (Waiting(..), &mut None) => {
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
    fn report(&mut self, peer: Id, schedule: ScheduleId) {
        if self.schedules.contains_key(&schedule) {
            return;
        }
        self.waiting.entry(schedule).or_insert_with(HashSet::new)
            .insert(peer);
    }
    fn get_state(&mut self) -> Vec<Arc<Schedule>> {
        self.schedules.drain().map(|(_, v)| v).collect()
    }
    fn poll_futures(&mut self, ctx: &mut Context) -> bool {
        struct ScheduleState {
            recent: Instant,
            fetching: HashSet<SocketAddr>,
        }

        let mut futures = HashMap::new();

        for _ in 0..self.fetching.len() {
            let (mut ctx, mach) = self.fetching.pop_front().unwrap();
            match mach.poll(&mut ctx) {
                Ok(VAsync::Ready(schedule)) => {
                    self.waiting.remove(&schedule.hash);
                    self.schedules.insert(schedule.hash.clone(), schedule);
                }
                Ok(VAsync::NotReady(mach)) => {
                    let entry = futures.entry(ctx.schedule.clone())
                        .or_insert_with(|| ScheduleState {
                            recent: ctx.started,
                            fetching: HashSet::new(),
                        });
                    entry.recent = max(entry.recent, ctx.started);
                    entry.fetching.insert(ctx.addr);
                    self.fetching.push_back((ctx, mach));
                }
                Err(e) => {
                    info!("Error fetching schedule {} from {}: {}",
                        ctx.schedule, ctx.addr, e);
                    self.blacklist.blacklist(ctx.addr,
                        Instant::now() + Duration::from_millis(
                            thread_rng().gen_range(
                                PREFETCH_BLACKLIST*1/2,
                                PREFETCH_BLACKLIST*3/2,
                                )));
                }
            }
        }

        let too_old = Instant::now() - Duration::from_millis(PREFETCH_OLD);
        let mut result = false;
        for (id, peers) in &self.waiting {
            let cur = if let Some(state) = futures.get(id) {
                if state.recent > too_old {
                    continue; // all ok
                }
                Some(&state.fetching)
            } else {
                None
            };
            // Issue a new connection
            let addr = sample(&mut thread_rng(), peers.iter()
                .filter_map(|id| get_addr(&ctx.shared, id))
                .filter(|&a| !self.blacklist.is_failing(a))
                .filter(|a| cur.map(|s| !s.contains(a)).unwrap_or(true)),
                1);
            if let Some(addr) = addr.into_iter().next() {
                self.fetching.push_back((FetchContext {
                    started: Instant::now(),
                    schedule: id.clone(),
                    addr: addr,
                    http_config: ctx.http_config.clone(),
                }, FetchState::Connecting(
                    TcpStream::connect(&addr, &handle()))));
                result = true;
            }
        }
        return result;
    }
}

impl StateMachine for FetchState {
    type Supply = FetchContext;
    type Item = Arc<Schedule>;
    type Error = ReplicaError;
    fn poll(mut self, ctx: &mut FetchContext)
        -> Result<VAsync<Self::Item, Self>, ReplicaError>
    {
        use self::FetchState::*;
        loop {
            self = match self {
                Connecting(mut conn) => match conn.poll() {
                    Ok(Async::Ready(sock)) => {
                        let proto = Proto::new(sock,
                            &handle(), &ctx.http_config);
                        WaitRequest(proto)
                    }
                    Ok(Async::NotReady) => {
                        return Ok(VAsync::NotReady(Connecting(conn)));
                    }
                    Err(e) => return Err(ReplicaError::Io(e)),
                },
                WaitRequest(mut proto) => {
                    let (codec, resp) = ReplicaCodec::new();
                    match proto.start_send(codec) {
                        Ok(AsyncSink::NotReady(_)) => {
                            // should be ready, but might be proto impl
                            // would change?
                            return Ok(VAsync::NotReady(WaitRequest(proto)));
                        }
                        Ok(AsyncSink::Ready) => WaitResponse(proto, resp),
                        Err(e) => return Err(ReplicaError::Http(e)),
                    }
                }
                WaitResponse(mut proto, mut chan) => {
                    let pro_poll = proto.poll_complete();
                    let chan_poll = chan.poll();
                    match (chan_poll, pro_poll) {
                        (Ok(Async::Ready(Ok(sched))), Ok(_)) => {
                            return Ok(VAsync::Ready(sched));
                        }
                        (Ok(Async::Ready(Err(e))), Ok(_)) => {
                            return Err(e);
                        }
                        (Ok(Async::NotReady), Ok(_)) => {
                            return Ok(VAsync::NotReady(
                                WaitResponse(proto, chan)));
                        }
                        (_, Err(e)) => {
                            debug!("Replica connection error: {}", e);
                            return Err(ReplicaError::Http(e));
                        }
                        (Err(e), _) => {
                            debug!("Request dropped unexpectedly: {}", e);
                            return Err(ReplicaError::OneshotCancel);
                        }
                    }
                }
            }
        }
    }
}

impl StateMachine for Prefetch {
    type Supply = Context;
    type Item = Vec<Arc<Schedule>>;
    type Error = Void;
    fn poll(mut self, ctx: &mut Context)
        -> Result<VAsync<Self::Item, Self>, Void>
    {
        while let Async::Ready(_) = self.blacklist.poll() { }
        while self.poll_futures(ctx) {
            while let Async::Ready(_) = self.blacklist.poll() { }
        }
        while let Async::Ready(_) = self.blacklist.poll() { }

        if self.state == PrefetchState::Fetching && self.waiting.len() == 0 {
            return Ok(VAsync::Ready(self.get_state()));
        }

        match self.timeout.poll().expect("timeout never fails") {
            Async::Ready(()) if self.state == PrefetchState::Graceful => {
                self.state = PrefetchState::Fetching;
                self.timeout = timeout_at(self.start +
                    Duration::from_millis(PREFETCH_MAX));
                match self.timeout.poll().expect("timeout never fails") {
                    Async::Ready(()) => {
                        return Ok(VAsync::Ready(self.get_state()));
                    }
                    Async::NotReady => {}
                }
            }
            Async::Ready(()) => {
                return Ok(VAsync::Ready(self.get_state()));
            }
            Async::NotReady => {}
        }
        Ok(VAsync::NotReady(self))
    }
}

pub fn spawn_fetcher(state: &SharedState,
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
    fn new() -> (ReplicaCodec, ScheduleRecv) {
        let (tx, rx) = oneshot::channel();
        (ReplicaCodec { tx: Some(tx) }, rx)
    }
}

impl Codec<TcpStream> for ReplicaCodec {
    type Future = FutureResult<EncoderDone<TcpStream>, Error>;
    fn start_write(&mut self, mut e: Encoder<TcpStream>) -> Self::Future {
        e.request_line("GET", "/v1/schedule", Version::Http11);
        // required by HTTP 1.1 spec
        e.add_header("Host", "verwalter").unwrap();
        e.done_headers().unwrap();
        ok(e.done())
    }
    fn headers_received(&mut self, headers: &Head) -> Result<RecvMode, Error> {
        if headers.status() == Some(Status::Ok) {
            Ok(RecvMode::buffered(10 << 20))
        } else {
            self.tx.take().expect("data_received called once")
                .send(Err(ReplicaError::InvalidStatus(headers.status())))
                .ok();
            Err(Error::custom("Invalid status"))
        }
    }
    fn data_received(
        &mut self,
        data: &[u8],
        end: bool
    ) -> Result<Async<usize>, Error> {
        assert!(end);
        self.tx.take().expect("data_received called once").send(
            serde_json::from_slice(data).map_err(ReplicaError::SerdeError)
            .and_then(|json| {
                scheduler::from_json(json)
                    .map(Arc::new)
                    .map_err(ReplicaError::ScheduleError)
            })).ok();
        Ok(Async::Ready(data.len()))
    }
}
