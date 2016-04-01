use std::io::{Read, Write};
use std::str::FromStr;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, Condvar, MutexGuard};
use std::time::Duration;
use std::collections::HashMap;

use time::{SteadyTime, Timespec, Duration as Dur};
use rotor::{Time, Notifier};
use cbor::{Encoder, EncodeResult, Decoder, DecodeResult};
use rustc_serialize::hex::{FromHex, ToHex, FromHexError};
use rustc_serialize::{Encodable, Encoder as RustcEncoder};

use config::Config;
use elect::{ElectionState, ScheduleStamp, Epoch};
use scheduler::{self, Schedule, PrefetchInfo, MAX_PREFETCH_TIME};


/// Things that are shared across the application threads
///
/// WARNING: this lock is a subject of Global Lock Ordering.
/// This lock should be help FIRST to the
///   LeaderState::Prefeching(Mutex<..>)
///
/// Currently this is the only constraint, but we are not sure it holds.
///
/// There are design patterns we obey here to hold that to true:
///
/// 1. We do our best to make this shared state a collection of Arcs so
///    you can pick up the arc'd object and work with it instead of holding
///    the lock
/// 2. We only hold this lock in inherent methods of the SharedState and
///    return only Arc'd values from here.
/// 3. All the dependencies in this structure that needs modify multiple
///    things in state at once are encoded here
/// 4. Keep all inherent methods FAST! So it's fine that they hold such
///    coarse-grained lock
///
#[derive(Clone)]
pub struct SharedState(Arc<Mutex<State>>, Arc<Notifiers>);

pub struct LeaderCookie {
    epoch: Epoch,
    pub parent_schedules: Vec<Arc<Schedule>>,
}


#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Id(Box<[u8]>);

impl Id {
    pub fn new<S:AsRef<[u8]>>(id: S) -> Id {
        Id(id.as_ref().to_owned().into_boxed_slice())
    }
    pub fn encode_cbor<W: Write>(&self, enc: &mut Encoder<W>) -> EncodeResult {
        enc.bytes(&self.0[..])
    }
    pub fn decode<R: Read>(dec: &mut Decoder<R>) -> DecodeResult<Id> {
        dec.bytes().map(|x| x.into_boxed_slice()).map(Id)
    }
    pub fn to_hex(&self) -> String {
        return self.0[..].to_hex();
    }
}

impl Encodable for Id {
    fn encode<S: RustcEncoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.to_hex().encode(s)
    }
}

impl FromStr for Id {
    type Err = FromHexError;
    fn from_str(s: &str) -> Result<Id, Self::Err> {
        s.from_hex().map(|x| x.into_boxed_slice()).map(Id)
    }
}

impl ::std::fmt::Display for Id {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(fmt, "{}", self.0.to_hex())
    }
}

impl ::std::fmt::Debug for Id {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(fmt, "Id({})", self.0.to_hex())
    }
}

#[derive(Clone, Debug)]
pub struct Peer {
     pub addr: Option<SocketAddr>,
     pub hostname: String,
     pub last_report: Option<Timespec>,
}

#[derive(Debug)]
struct State {
    config: Arc<Config>,
    peers: Option<Arc<(Time, HashMap<Id, Peer>)>>,
    last_known_schedule: Option<Arc<Schedule>>,
    // TODO(tailhook) rename schedule -> scheduleR
    schedule: Arc<scheduler::State>,
    election: Arc<ElectionState>,
    /// Fetch update notifier
    external_schedule_update: Option<Notifier>,
}

struct Notifiers {
    apply_schedule: Condvar,
    run_scheduler: Condvar,
}

fn stable_schedule(guard: &mut MutexGuard<State>) -> Option<Arc<Schedule>> {
    use scheduler::State::{Following, Leading};
    use scheduler::FollowerState as F;
    use scheduler::LeaderState as L;
    match *guard.schedule {
        Following(_, F::Stable(ref x)) => Some(x.clone()),
        Leading(L::Stable(ref x)) => Some(x.clone()),
        _ => None,
    }
}

fn fetch_schedule(guard: &mut MutexGuard<State>) {
    guard.external_schedule_update.as_ref()
        .map(|x| x.wakeup().expect("send fetch schedule notification"));
}

impl SharedState {
    pub fn new(cfg: Config) -> SharedState {
        SharedState(Arc::new(Mutex::new(State {
            config: Arc::new(cfg),
            peers: None,
            schedule: Arc::new(scheduler::State::Unstable),
            last_known_schedule: None,
            election: Default::default(),
            external_schedule_update: None, //unfortunately
        })), Arc::new(Notifiers {
            apply_schedule: Condvar::new(),
            run_scheduler: Condvar::new(),
        }))
    }
    fn lock(&self) -> MutexGuard<State> {
        self.0.lock().expect("shared state lock")
    }
    // Accessors
    pub fn peers(&self) -> Option<Arc<(Time, HashMap<Id, Peer>)>> {
        self.lock().peers.clone()
    }
    pub fn config(&self) -> Arc<Config> {
        self.0.lock().expect("shared state lock").config.clone()
    }
    /// Returns last known schedule
    pub fn schedule(&self) -> Option<Arc<Schedule>> {
        self.0.lock().expect("shared state lock").last_known_schedule.clone()
    }
    pub fn scheduler_state(&self) -> Arc<scheduler::State> {
        self.lock().schedule.clone()
    }
    pub fn stable_schedule(&self) -> Option<Arc<Schedule>> {
        stable_schedule(&mut self.lock())
    }
    pub fn owned_schedule(&self) -> Option<Arc<Schedule>> {
        use scheduler::State::Leading;
        use scheduler::LeaderState::Stable;
        match *self.lock().schedule {
            Leading(Stable(ref x)) => Some(x.clone()),
            _ => None,
        }
    }
    pub fn election(&self) -> Arc<ElectionState> {
        self.0.lock().expect("shared state lock").election.clone()
    }
    pub fn should_schedule_update(&self) -> bool {
        use scheduler::State::{Following};
        use scheduler::FollowerState::Fetching;
        match *self.lock().schedule {
            Following(_, Fetching(..)) => true,
            _ => false,
        }
    }
    // Setters
    pub fn set_peers(&self, time: Time, peers: HashMap<Id, Peer>) {
        let mut guard = self.lock();
        guard.peers = Some(Arc::new((time, peers)));

        // TODO(tailhook) should we bother to run it every time?
        //
        // Maybe either:
        //
        // 1. Notify when first data available
        // 2. Notify when anything changed
        //
        // Note: while comparison is definitely cheaper than a new scheduling
        // but we should compare smartly. I.e. peers are always changed (i.e.
        // ping timestamps and similar things). We should check for meaningful
        // changes.
        self.1.run_scheduler.notify_all();
    }
    #[allow(unused)]
    pub fn set_config(&self, cfg: Config) {
        self.0.lock().expect("shared state lock").config = Arc::new(cfg);
    }
    pub fn set_schedule_by_leader(&self, val: Schedule) {
        use scheduler::State::{Leading};
        use scheduler::LeaderState::{Stable, Calculating};
        let mut guard = self.lock();
        match *guard.schedule {
            Leading(Calculating) => {
                let sched = Arc::new(val);
                guard.schedule = Arc::new(Leading(Stable(sched.clone())));
                guard.last_known_schedule = Some(sched);
                self.1.apply_schedule.notify_all();
            }
            _ => {
                debug!("Calculated a schedule when not a leader already");
            }
        }
    }
    // TODO(tailhook) this method does too much, refactor it
    pub fn update_election(&self, elect: ElectionState,
                            peer_schedule: Option<(Id, ScheduleStamp)>)
    {
        use scheduler::State::*;
        use scheduler::LeaderState::Prefetching;
        use scheduler::FollowerState::*;
        let mut guard = self.lock();
        if !elect.is_stable {
            if !matches!(*guard.schedule, Unstable) {
                guard.schedule = Arc::new(Unstable);
            }
        } else if elect.is_leader {
            match *guard.schedule.clone() {
                Unstable | Following(..) => {
                    let empty_map = HashMap::new();
                    let mut initial = PrefetchInfo::new(
                        guard.peers.as_ref()
                        .map(|x| &x.1).unwrap_or(&empty_map)
                        .keys().cloned());
                    peer_schedule.map(|(id, stamp)| {
                        initial.peer_report(id, stamp)
                    });
                    guard.schedule = Arc::new(
                        Leading(Prefetching(SteadyTime::now(),
                                            Mutex::new(initial))));
                    fetch_schedule(&mut guard);
                }
                Leading(Prefetching(_, ref pref)) => {
                    peer_schedule.map(|(id, stamp)| {
                        let mut p = pref.lock().expect("prefetching lock");
                        if p.peer_report(id, stamp) {
                            fetch_schedule(&mut guard);
                        }
                    });
                }
                Leading(..) => { }
            }
        } else {
            match *guard.schedule.clone() {
                Following(ref id, ref status)
                if Some(id) == elect.leader.as_ref()
                => {
                    if let Some((schid, tstamp)) = peer_schedule {
                        debug_assert!(id == &schid);
                        match *status {
                            Stable(ref schedule)
                            if schedule.hash == tstamp.hash
                            => {}  // up to date
                            Fetching(ref hash) if hash == &tstamp.hash
                            => {}  // already fetching
                            _ => {
                                guard.schedule = Arc::new(Following(
                                    id.clone(),
                                    Fetching(tstamp.hash)));
                                fetch_schedule(&mut guard);
                            }
                        }
                    }
                }
                _ => {
                    guard.schedule = Arc::new(Following(
                        elect.leader.clone().unwrap(),
                        match peer_schedule {
                            Some((schid, x)) => {
                                debug_assert!(elect.leader.as_ref() ==
                                              Some(&schid));
                                Fetching(x.hash)
                            }
                            None => Waiting,
                        }));
                    fetch_schedule(&mut guard);
                }
            }

        }
        guard.election = Arc::new(elect);
    }
    // Utility

    pub fn wait_schedule_update(&self, max_interval: Duration)
        -> LeaderCookie
    {
        use scheduler::State::*;
        use scheduler::LeaderState::*;
        let mut guard = self.lock();
        let mut wait_time = max_interval;
        loop {
            guard = self.1.run_scheduler
                .wait_timeout(guard, wait_time)
                .expect("shared state lock")
                .0;
            if guard.peers.is_none() {
                // Don't care even checking anything if we have no peers
                continue;
            }
            wait_time = match *guard.schedule.clone() {
                Leading(Prefetching(time, ref mutex)) => {
                    let time_left = time + Dur::milliseconds(MAX_PREFETCH_TIME)
                        - SteadyTime::now();
                    if time_left <= Dur::zero() ||
                        mutex.lock().expect("prefetch lock").done()
                    {
                        guard.schedule = Arc::new(Leading(Calculating));
                        return LeaderCookie {
                            epoch: guard.election.epoch,
                            parent_schedules:
                                mutex.lock().expect("prefetch lock")
                                .get_schedules(),
                        };
                    } else {
                        Duration::from_millis(
                            time_left.num_milliseconds() as u64)
                    }
                }
                Leading(Stable(ref x)) => {
                    guard.schedule = Arc::new(Leading(Calculating));
                    return LeaderCookie {
                        epoch: guard.election.epoch,
                        parent_schedules: vec![x.clone()],
                    };
                }
                Leading(Calculating) => unreachable!(),
                _ => max_interval,
            };
        }
    }
    pub fn is_cookie_valid(&self, cookie: &LeaderCookie) -> bool {
        cookie.epoch == self.lock().election.epoch
    }

    /// This is waited on in apply/render code
    pub fn wait_new_schedule(&self, hash: &str) -> Arc<Schedule> {
        let mut guard = self.lock();
        loop {
            match stable_schedule(&mut guard) {
                Some(schedule) => {
                    if &schedule.hash != &hash { // only if not up to date
                        return schedule;
                    }
                }
                None => {}
            };
            guard = self.1.apply_schedule.wait(guard)
                .expect("shared state lock");
        }
    }
    pub fn fetched_schedule(&self, schedule: Schedule) {
        use scheduler::State::{Following, Leading};
        use scheduler::FollowerState::{Fetching, Stable};
        use scheduler::LeaderState::{Prefetching};
        let ref mut guard = *self.lock();
        match *guard.schedule.clone() {
            Following(ref id, Fetching(ref hash)) if &schedule.hash == hash
            => {
                let sched = Arc::new(schedule);
                guard.schedule = Arc::new(
                    Following(id.clone(), Stable(sched.clone())));
                guard.last_known_schedule = Some(sched);
                self.1.apply_schedule.notify_all();
            }
            Leading(Prefetching(_, ref mutex)) => {
                let mut lock = mutex.lock().expect("prefetch lock");
                lock.add_schedule(schedule);
                if lock.done() {
                    self.1.run_scheduler.notify_all();
                }
            }
            _ => {
                debug!("Received outdated schedule");
            }
        }
    }
    pub fn set_update_notifier(&self, notifier: Notifier) {
        let mut guard = self.0.lock().expect("shared state lock");
        guard.external_schedule_update = Some(notifier);
    }
}
