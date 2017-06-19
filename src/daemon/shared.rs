use std::io::{Read, Write};
use std::ops::Deref;
use std::str::FromStr;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, Condvar, MutexGuard};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, Instant};
use std::collections::{HashMap, BTreeMap, HashSet};
use std::collections::btree_map::Entry::{Occupied, Vacant};

use futures::Future;
use futures::sync::oneshot;
use tokio_core::reactor::Remote;
use time::{SteadyTime, Timespec, Duration as Dur, get_time};
use cbor::{Encoder, EncodeResult, Decoder, DecodeResult};
use rustc_serialize::hex::{FromHex, ToHex, FromHexError};
use rustc_serialize::{Encodable, Encoder as RustcEncoder};
use serde_json::Value as Json;

use cell;
use config::Sandbox;
use elect::{ElectionState, ScheduleStamp, Epoch};
use id::Id;
use {Options};
use peer::Peer;
use prefetch::PrefetchStatus;
use replica::Replica;
use scheduler::{self, Schedule, PrefetchInfo, MAX_PREFETCH_TIME, SchedulerInput};
use time_util::ToMsec;


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
pub struct SharedState(Arc<SharedData>, Arc<Mutex<State>>);

pub struct SharedData {
    pub id: Id,
    pub name: String,
    pub hostname: String,
    pub options: Options,
    pub sandbox: Sandbox,
    pub mainloop: Remote,
    force_render: AtomicBool,
    apply_schedule: Condvar,
    run_scheduler: Condvar,
    peers: cell::Sender<Arc<(SystemTime, HashMap<Id, Peer>)>>,
}

pub struct LeaderCookie {
    epoch: Epoch,
    pub parent_schedules: Vec<Arc<Schedule>>,
    pub actions: BTreeMap<u64, Arc<Json>>,
}

pub enum PushActionError {
    TooManyRequests,
    NotALeader,
}

#[derive(Debug)]
struct State {
    last_known_schedule: Option<Arc<Schedule>>,
    // TODO(tailhook) rename schedule -> scheduleR
    schedule: Arc<scheduler::State>,
    last_scheduler_debug_info: Arc<Option<(SchedulerInput, String)>>,
    election: Arc<ElectionState>,
    actions: BTreeMap<u64, Arc<Json>>,
    errors: Arc<HashMap<&'static str, String>>,
    failed_roles: Arc<HashSet<String>>,
    replica_shutter: Option<oneshot::Sender<()>>,
    prefetch_status: Option<PrefetchStatus>,
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

impl Deref for SharedState {
    type Target = SharedData;
    fn deref(&self) -> &SharedData {
        return &self.0;
    }
}

impl SharedState {
    pub fn new(id: &Id, name: &str, hostname: &str,
               options: Options, sandbox: Sandbox,
               old_schedule: Option<Schedule>, mainloop: &Remote)
        -> SharedState
    {
        SharedState(
            Arc::new(SharedData {
                id: id.clone(),
                name: name.to_string(),
                hostname: hostname.to_string(),
                options,
                sandbox,
                force_render: AtomicBool::new(false),
                apply_schedule: Condvar::new(),
                run_scheduler: Condvar::new(),
                peers: cell::Sender::new(
                    Arc::new((SystemTime::now(), HashMap::new()))),
                mainloop: mainloop.clone(),
            }),
            Arc::new(Mutex::new(State {
                schedule: Arc::new(scheduler::State::Unstable),
                last_known_schedule: old_schedule.map(Arc::new),
                last_scheduler_debug_info: Arc::new(None),
                election: Arc::new(ElectionState::blank()),
                actions: BTreeMap::new(),
                errors: Arc::new(HashMap::new()),
                failed_roles: Arc::new(HashSet::new()),
                replica_shutter: None,
                prefetch_status: None, // TODO(tailhook) move to state
            })),
        )
    }
    fn lock(&self) -> MutexGuard<State> {
        self.1.lock().expect("shared state lock")
    }
    // Accessors
    pub fn id(&self) -> &Id {
        &self.id
    }
    pub fn debug_force_leader(&self) -> bool {
        self.options.debug_force_leader
    }
    pub fn peers(&self) -> Arc<(SystemTime, HashMap<Id, Peer>)> {
        self.peers.get()
    }
    pub fn peer_cell(&self)
        -> cell::Cell<Arc<(SystemTime, HashMap<Id, Peer>)>>
    {
        self.peers.cell()
    }
    /// Returns last known schedule
    pub fn schedule(&self) -> Option<Arc<Schedule>> {
        self.lock().last_known_schedule.clone()
    }
    pub fn scheduler_debug_info(&self) -> Arc<Option<(SchedulerInput, String)>>
    {
        self.lock().last_scheduler_debug_info.clone()
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
        self.lock().election.clone()
    }
    pub fn should_schedule_update(&self) -> bool {
        use scheduler::State::{Following};
        use scheduler::FollowerState::Fetching;
        match *self.lock().schedule {
            Following(_, Fetching(..)) => true,
            _ => false,
        }
    }
    pub fn pending_actions(&self) -> BTreeMap<u64, Arc<Json>> {
        self.lock().actions.clone()
    }
    pub fn errors(&self) -> Arc<HashMap<&'static str, String>> {
        self.lock().errors.clone()
    }
    pub fn failed_roles(&self) -> Arc<HashSet<String>> {
        self.lock().failed_roles.clone()
    }
    /*
    pub fn metrics(&self) -> Option<Arc<RemoteQuery>> {
        self.lock().cantal.as_ref().unwrap().get_remote_query()
    }
    */
    // Setters
    pub fn set_peers(&self, time: SystemTime, peers: HashMap<Id, Peer>) {
        self.peers.set(Arc::new((time, peers)));

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
        //
        // **UPDATE** don't rerun scheduler on updated peers, scheduler will
        // notice it on next normal wakeup of ~5 seconds
        //
        // self.0.run_scheduler.notify_all();
    }
    pub fn set_schedule_debug_info(&self, input: SchedulerInput, debug: String)
    {
        self.lock().last_scheduler_debug_info =
            Arc::new(Some((input, debug)));
    }
    pub fn set_schedule_by_leader(&self, cookie: LeaderCookie,
        val: Schedule, input: SchedulerInput, debug: String)
    {
        use scheduler::State::{Leading};
        use scheduler::LeaderState::{Stable, Calculating};
        let mut guard = self.lock();
        match *guard.schedule {
            Leading(Calculating) => {
                /*
                if let Some(ref x) = val.data.find("query_metrics") {
                    guard.cantal.as_ref().unwrap()
                        .set_remote_query_json(x, Duration::new(5, 0));
                } else {
                    guard.cantal.as_ref().unwrap().clear_remote_query();
                }
                */
                let sched = Arc::new(val);
                guard.schedule = Arc::new(Leading(Stable(sched.clone())));
                guard.last_known_schedule = Some(sched);
                guard.last_scheduler_debug_info =
                    Arc::new(Some((input, debug)));
                for (aid, _) in cookie.actions {
                    guard.actions.remove(&aid);
                }
                self.apply_schedule.notify_all();
            }
            _ => {
                debug!("Calculated a schedule when not a leader already");
            }
        }
    }
    pub fn set_error(&self, domain: &'static str, value: String) {
        Arc::make_mut(&mut self.lock().errors).insert(domain, value);
    }
    pub fn clear_error(&self, domain: &'static str) {
        Arc::make_mut(&mut self.lock().errors).remove(domain);
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
            guard.actions.clear();
            guard.replica_shutter.take(); // destroys/stops shutter if exists
            if !matches!(*guard.schedule, Unstable) {
                guard.schedule = Arc::new(Unstable);
            }
        } else if elect.is_leader {
            guard.replica_shutter.take(); // destroys/stops shutter if exists
            match *guard.schedule.clone() {
                Unstable | Following(..) => {
                    let mut initial = PrefetchInfo::new(
                        self.0.peers.get().1.keys().cloned(),
                        guard.last_known_schedule.clone());
                    peer_schedule.map(|(id, stamp)| {
                        initial.peer_report(id, stamp)
                    });
                    guard.schedule = Arc::new(
                        Leading(Prefetching(Instant::now(),
                                            Mutex::new(initial))));
                    guard.prefetch_status = Some(PrefetchStatus::new(self));
                }
                Leading(Prefetching(_, ref pref)) => {
                    peer_schedule.map(|(id, stamp)| {
                        let mut p = pref.lock().expect("prefetching lock");
                        if p.peer_report(id, stamp) {
                            //fetch_schedule(&mut guard);
                            unimplemented!();
                        }
                    });
                }
                Leading(..) => { }
            }
        } else { // is a follower
            guard.actions.clear();
            guard.prefetch_status.take(); // destroys/stops prefetch
            match *guard.schedule.clone() {
                Following(ref id, ref status)
                if Some(id) == elect.leader.as_ref()
                => {
                    if let Some((schid, tstamp)) = peer_schedule {
                        if id == &schid {
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
                                    //fetch_schedule(&mut guard);
                                    unimplemented!();
                                }
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
                    //fetch_schedule(&mut guard);
                    unimplemented!();
                }
            }

        }
        if !elect.is_leader {
            //guard.cantal.as_ref().unwrap().clear_remote_query();
            let errors = Arc::make_mut(&mut guard.errors);
            errors.remove("reload_configs");
            errors.remove("scheduler_load");
            errors.remove("scheduler");
        }
        let mut elect = elect;
        elect.last_stable_timestamp = elect.last_stable_timestamp
            .or(guard.election.last_stable_timestamp);
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
            guard = self.run_scheduler
                .wait_timeout(guard, wait_time)
                .expect("shared state lock")
                .0;
            wait_time = match *guard.schedule.clone() {
                Leading(Prefetching(time, ref mutex)) => {
                    let pref = Duration::from_millis(MAX_PREFETCH_TIME);
                    let elapsed = time.elapsed();
                    if elapsed > pref ||
                        mutex.lock().expect("prefetch lock").done()
                    {
                        guard.schedule = Arc::new(Leading(Calculating));
                        return LeaderCookie {
                            epoch: guard.election.epoch,
                            parent_schedules:
                                mutex.lock().expect("prefetch lock")
                                .get_schedules(),
                            actions: guard.actions.clone(),
                        };
                    } else {
                        pref - elapsed
                    }
                }
                Leading(Stable(ref x)) => {
                    guard.schedule = Arc::new(Leading(Calculating));
                    return LeaderCookie {
                        epoch: guard.election.epoch,
                        parent_schedules: vec![x.clone()],
                        actions: guard.actions.clone(),
                    };
                }
                Leading(Calculating) => unreachable!(),
                _ => max_interval,
            };
        }
    }
    pub fn refresh_cookie(&self, cookie: &mut LeaderCookie) -> bool {
        let guard = self.lock();
        if cookie.epoch == guard.election.epoch {
            // TODO(tailhook) update only changed items
            cookie.actions = guard.actions.clone();
            return true;
        } else {
            return false;
        }
    }

    /// This is waited on in apply/render code
    pub fn wait_new_schedule(&self, hash: &str) -> Arc<Schedule>
    {
        let mut guard = self.lock();
        loop {
            match stable_schedule(&mut guard) {
                Some(schedule) => {
                    if self.force_render.swap(false, SeqCst) ||  // if forced
                        &schedule.hash != &hash  // or not up to date
                    {
                        return schedule;
                    }
                }
                None => {}
            };
            guard = self.apply_schedule.wait(guard)
                .expect("shared state lock");
        }
    }
    pub fn force_render(&self) {
        self.force_render.store(true, SeqCst);
        self.apply_schedule.notify_all();
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
                if guard.last_scheduler_debug_info.is_some() {
                    guard.last_scheduler_debug_info = Arc::new(None);
                }
                guard.schedule = Arc::new(
                    Following(id.clone(), Stable(sched.clone())));
                guard.last_known_schedule = Some(sched);
                self.apply_schedule.notify_all();
            }
            Leading(Prefetching(_, ref mutex)) => {
                let mut lock = mutex.lock().expect("prefetch lock");
                lock.add_schedule(Arc::new(schedule));
                if lock.done() {
                    self.run_scheduler.notify_all();
                }
            }
            _ => {
                debug!("Received outdated schedule");
            }
        }
    }
    // late initializers
    /*
    pub fn set_update_notifier(&self, notifier: Notifier) {
        let mut guard = self.lock();
        guard.external_schedule_update = Some(notifier);
    }
    pub fn set_cantal(&self, cantal: Cantal) {
        let mut guard = self.lock();
        guard.cantal = Some(cantal);
    }
    */
    pub fn push_action(&self, data: Json) -> Result<u64, PushActionError> {
        use scheduler::State::{Following, Leading, Unstable};
        let mut guard = self.lock();

        match *guard.schedule.clone() {
            Unstable | Following(..) => {
                return Err(PushActionError::NotALeader);
            }
            Leading(..) => {}
        }

        let millis = (get_time().sec * 1000) as u64;

        // Note we intentionally limit actions to 1000 per second
        // Usually there is no more than *one*
        // TODO(tailhook) we can look at max element rather than iterating
        for i in 0..1000 {
            match guard.actions.entry(millis + i) {
                Occupied(_) => continue,
                Vacant(x) => {
                    x.insert(Arc::new(data));
                    return Ok(millis + i);
                }
            }
        }
        return Err(PushActionError::TooManyRequests);
    }
    pub fn check_action(&self, action: u64) -> bool {
        self.lock().actions.get(&action).is_some()
    }
    pub fn mark_role_failure(&self, role_name: &str) {
        let ref mut lock = self.lock();
        let ref mut role_errors = Arc::make_mut(&mut lock.failed_roles);
        if !role_errors.contains(role_name) {
            role_errors.insert(role_name.to_string());
        }
    }
    pub fn reset_role_failure(&self, role_name: &str) {
        Arc::make_mut (&mut self.lock().failed_roles).remove(role_name);
    }
}
