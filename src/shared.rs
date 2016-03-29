use std::io::{Read, Write};
use std::str::FromStr;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, Condvar, MutexGuard};
use std::collections::HashMap;

use time::Timespec;
use rotor::{Time, Notifier};
use cbor::{Encoder, EncodeResult, Decoder, DecodeResult};
use rustc_serialize::hex::{FromHex, ToHex, FromHexError};
use rustc_serialize::{Encodable, Encoder as RustcEncoder};

use config::Config;
use elect::{ElectionState, ScheduleStamp};
use scheduler::{self, Schedule, BuildInfo};


#[derive(Clone)]
pub struct SharedState(Arc<Mutex<State>>, Arc<Notifiers>);


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
        self.0.lock().expect("shared state lock")
            .peers = Some(Arc::new((time, peers)));
    }
    #[allow(unused)]
    pub fn set_config(&self, cfg: Config) {
        self.0.lock().expect("shared state lock").config = Arc::new(cfg);
    }
    pub fn set_schedule_by_leader(&self, val: Schedule) {
        use scheduler::State::{Leading};
        use scheduler::LeaderState::Stable;
        let mut guard = self.lock();
        let sched = Arc::new(val);
        // TODO(tailhook) should we check current scheduling state?
        // We definitely should!!!
        guard.schedule = Arc::new(Leading(Stable(sched.clone())));
        guard.last_known_schedule = Some(sched);
        self.1.apply_schedule.notify_all();
    }
    // TODO(tailhook) this method does too much, refactor it
    pub fn update_election(&self, elect: ElectionState,
                            peer_schedule: Option<ScheduleStamp>)
    {
        use scheduler::State::*;
        use scheduler::LeaderState::Building;
        use scheduler::FollowerState::*;
        let mut guard = self.lock();
        if !elect.is_stable {
            if !matches!(*guard.schedule, Unstable) {
                guard.schedule = Arc::new(Unstable);
            }
        } else if elect.is_leader {
            match *guard.schedule.clone() {
                Unstable | Following(..) => {
                    let mut initial = BuildInfo::new();
                    // TODO(tailhook) add contained data
                    guard.schedule = Arc::new(
                        Leading(Building(Mutex::new(initial))));
                }
                Leading(..) => { }
            }
        } else {
            match *guard.schedule.clone() {
                Following(ref id, ref status)
                if Some(id) == elect.leader.as_ref()
                => {
                    if let Some(tstamp) = peer_schedule {
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
                            Some(x) => Fetching(x.hash),
                            None => Waiting,
                        }));
                    fetch_schedule(&mut guard);
                }
            }

        }
        guard.election = Arc::new(elect);
    }
    // Utility

    /// Returns true if we are leader and sets state to Calculating
    pub fn start_schedule_update(&self) -> bool {
        use scheduler::State::*;
        use scheduler::LeaderState::*;
        let mut guard = self.lock();
        match *guard.schedule.clone() {
            Leading(Building(..)) => {
                unimplemented!();
            }
            Leading(Calculating) => true,
            Leading(Stable(..)) => {
                guard.schedule = Arc::new(Leading(Calculating));
                true
            }
            _ => false,
        }
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
    pub fn set_schedule_if_matches(&self, schedule: Schedule) {
        use scheduler::State::{Following};
        use scheduler::FollowerState::{Fetching, Stable};
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
