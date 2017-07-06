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
use crossbeam::sync::ArcCell;

use cell;
use config::Sandbox;
use elect::{ElectionState, ScheduleStamp, Epoch};
use fetch;
use id::Id;
use {Options};
use peer::{Peer, Peers};
use scheduler::{self, Schedule, MAX_PREFETCH_TIME, SchedulerInput};
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
    pub fetch_state: ArcCell<fetch::PublicState>,
    force_render: AtomicBool,
    apply_schedule: Condvar,
    run_scheduler: Condvar,
    peers: ArcCell<Peers>,
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
    last_scheduler_debug_info: Arc<Option<(SchedulerInput, String)>>,
    election: Arc<ElectionState>,
    actions: BTreeMap<u64, Arc<Json>>,
    errors: Arc<HashMap<&'static str, String>>,
    failed_roles: Arc<HashSet<String>>,
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
                peers: ArcCell::new(Arc::new(Peers::new())),
                mainloop: mainloop.clone(),
                fetch_state: ArcCell::new(
                    Arc::new(fetch::PublicState::Unstable)),
            }),
            Arc::new(Mutex::new(State {
                last_known_schedule: old_schedule.map(Arc::new),
                last_scheduler_debug_info: Arc::new(None),
                election: Arc::new(ElectionState::blank()),
                actions: BTreeMap::new(),
                errors: Arc::new(HashMap::new()),
                failed_roles: Arc::new(HashSet::new()),
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
    pub fn peers(&self) -> Arc<Peers> {
        self.peers.get()
    }
    /// Returns last known schedule
    pub fn schedule(&self) -> Option<Arc<Schedule>> {
        self.lock().last_known_schedule.clone()
    }
    pub fn scheduler_debug_info(&self) -> Arc<Option<(SchedulerInput, String)>>
    {
        self.lock().last_scheduler_debug_info.clone()
    }
    pub fn election(&self) -> Arc<ElectionState> {
        self.lock().election.clone()
    }
    pub fn stable_schedule(&self) -> Option<Arc<Schedule>> {
        unimplemented!();
    }
    pub fn owned_schedule(&self) -> Option<Arc<Schedule>> {
        unimplemented!();
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
        let old_peers = self.peers.get();
        let mut to_insert = Vec::new();
        for (id, peer) in &peers {
            if let Some(ref mut old) = old_peers.peers.get(&id) {
                let oldp = old.get();
                if oldp.addr != peer.addr ||
                   oldp.name != peer.name ||
                   oldp.hostname != peer.hostname
                {
                    let new = Peer {
                        schedule: oldp.schedule.clone(),
                        .. peer.clone()
                    };
                    old.set(Arc::new(new));
                }
            } else {
                to_insert.push((id, peer));
            }
        }
        let mut needs_mutation = to_insert.len() > 0;
        if !needs_mutation {
            for (id, _) in &old_peers.peers {
                if !peers.contains_key(id) {
                    needs_mutation = true;
                }
            }
        }
        if !needs_mutation {
            return;
        }
        let mut new_peers = HashMap::new();
        for (id, peer) in &old_peers.peers {
            if peers.contains_key(id) {
                new_peers.insert(id.clone(), ArcCell::new(peer.get()));
            }
        }
        for (id, peer) in to_insert {
            new_peers.insert(id.clone(), ArcCell::new(Arc::new(peer.clone())));
        }
        self.peers.set(Arc::new(Peers {
            timestamp: time,
            peers: new_peers,
        }));
    }
    pub fn set_schedule_debug_info(&self, input: SchedulerInput, debug: String)
    {
        self.lock().last_scheduler_debug_info =
            Arc::new(Some((input, debug)));
    }
    pub fn set_schedule_by_leader(&self, cookie: LeaderCookie,
        val: Schedule, input: SchedulerInput, debug: String)
    {
        unimplemented!();
        /*
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
        */
    }
    pub fn set_error(&self, domain: &'static str, value: String) {
        Arc::make_mut(&mut self.lock().errors).insert(domain, value);
    }
    pub fn clear_error(&self, domain: &'static str) {
        Arc::make_mut(&mut self.lock().errors).remove(domain);
    }
    // TODO(tailhook) this method does too much, refactor it
    pub fn update_election(&self, mut elect: ElectionState) {
        let mut guard = self.lock();
        if !elect.is_leader {
            guard.actions.clear();
            let errors = Arc::make_mut(&mut guard.errors);
            errors.remove("reload_configs");
            errors.remove("scheduler_load");
            errors.remove("scheduler");
        }
        let mut dest_elect = Arc::make_mut(&mut guard.election);
        let tstamp = elect.last_stable_timestamp
            .or(dest_elect.last_stable_timestamp);
        *dest_elect = elect;
        dest_elect.last_stable_timestamp = tstamp;
    }
    // Utility

    pub fn wait_schedule_update(&self, max_interval: Duration)
        -> LeaderCookie
    {
        unimplemented!();
        /*
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
                Leading(Prefetching) => {
                    unimplemented!();
                    /*
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
                    */
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
        */
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
        unimplemented!();
        /*
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
        */
    }
    pub fn force_render(&self) {
        self.force_render.store(true, SeqCst);
        self.apply_schedule.notify_all();
    }
    /*
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
    */
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
        unimplemented!();
        /*
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
        */
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
