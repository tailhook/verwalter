use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime};
use std::collections::{HashMap, BTreeMap, HashSet};
use std::collections::btree_map::Entry::{Occupied, Vacant};

use async_slot as slot;
use tokio_core::reactor::Remote;
use time::{get_time};
use serde_json::Value as Json;
use crossbeam::sync::ArcCell;

use config::Sandbox;
use elect::{ElectionState, Epoch};
use fetch;
use id::Id;
use {Options};
use peer::{Peer, Peers};
use metrics::{Integer, List, Counter, Metric};
use scheduler::{Schedule, SchedulerInput, ScheduleId};

lazy_static! {
    static ref ACTIONS_EXECUTED: Counter = Counter::new();
    static ref FAILED_ROLES: Counter = Counter::new();
    static ref ERRORED: Counter = Counter::new();
    static ref ERRORS: Integer = Integer::new();
    static ref FAILING_ROLES: Integer = Integer::new();
    static ref PEERS: Integer = Integer::new();
}

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
    peers: ArcCell<Peers>,
    schedule_channel: slot::Sender<Arc<Schedule>>,
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
    stable_schedule: Option<Arc<Schedule>>,
    owned_schedule: Option<Arc<Schedule>>,
    last_scheduler_debug_info: Arc<Option<(SchedulerInput, String)>>,
    election: Arc<ElectionState>,
    actions: BTreeMap<u64, Arc<Json>>,
    errors: Arc<HashMap<&'static str, String>>,
    failed_roles: Arc<HashSet<String>>,
    // TODO(tailhook) it's a bit ugly that parents used only once, are
    // stored here
    parent_schedules: Option<Vec<Arc<Schedule>>>,
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
               old_schedule: Option<Schedule>,
               schedule_channel: slot::Sender<Arc<Schedule>>,
               mainloop: &Remote)
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
                peers: ArcCell::new(Arc::new(Peers::new())),
                mainloop: mainloop.clone(),
                fetch_state: ArcCell::new(
                    Arc::new(fetch::PublicState::Unstable)),
                schedule_channel,
            }),
            Arc::new(Mutex::new(State {
                last_known_schedule: old_schedule.map(Arc::new),
                last_scheduler_debug_info: Arc::new(None),
                election: Arc::new(ElectionState::blank()),
                actions: BTreeMap::new(),
                errors: Arc::new(HashMap::new()),
                failed_roles: Arc::new(HashSet::new()),
                stable_schedule: None,
                owned_schedule: None,
                parent_schedules: None,
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
        self.lock().stable_schedule.clone()
    }
    pub fn is_current(&self, hash: &ScheduleId) -> bool {
        self.lock().stable_schedule.as_ref().map(|x| &x.hash) == Some(hash)
    }
    pub fn owned_schedule(&self) -> Option<Arc<Schedule>> {
        self.lock().owned_schedule.clone()
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
    // Setters
    pub fn set_peers(&self, time: SystemTime, peers: HashMap<Id, Peer>) {
        // all this logic seems to be very ugly
        // TODO(tailhook) find some simpler way
        let old_peers = self.peers.get();
        let mut to_insert = Vec::new();
        for (id, peer) in &peers {
            if let Some(ref mut old) = old_peers.peers.get(&id) {
                let oldp = old.get();
                if oldp.needs_refresh(&peer) {
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
        let mut new_peers = HashMap::new();
        for (id, peer) in &old_peers.peers {
            if peers.contains_key(id) {
                new_peers.insert(id.clone(), ArcCell::new(peer.get()));
            }
        }
        for (id, peer) in to_insert {
            new_peers.insert(id.clone(), ArcCell::new(Arc::new(peer.clone())));
        }
        PEERS.set(new_peers.len() as i64);
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
    pub fn set_schedule_by_follower(&self, schedule: &Arc<Schedule>)
    {
        let mut guard = self.lock();
        if !guard.election.is_leader &&
            guard.election.leader.as_ref() == Some(&schedule.origin)
        {
            guard.last_known_schedule = Some(schedule.clone());
            guard.stable_schedule = Some(schedule.clone());
            self.0.schedule_channel.swap(schedule.clone())
                .expect("apply channel is alive");
        } else {
            debug!("Ingoring follower schedule {} from {}",
                schedule.hash, schedule.origin);
        }
    }
    pub fn reset_stable_schedule(&self) {
        // owned schedule is reset in update_election handler
        self.lock().stable_schedule = None
    }
    pub fn set_schedule_by_leader(&self, cookie: LeaderCookie,
        val: Schedule, input: SchedulerInput, debug: String)
    {
        let mut guard = self.lock();
        if guard.election.is_leader && guard.election.epoch == cookie.epoch {
            for action in cookie.actions.keys() {
                guard.actions.remove(action);
            }
            let schedule = Arc::new(val);
            guard.last_known_schedule = Some(schedule.clone());
            guard.owned_schedule = Some(schedule.clone());
            guard.stable_schedule = Some(schedule.clone());
            guard.last_scheduler_debug_info = Arc::new(Some((input, debug)));
            self.0.schedule_channel.swap(schedule.clone())
                .expect("apply channel is alive");
        }
    }
    pub fn set_error(&self, domain: &'static str, value: String) {
        let mut lock = self.lock();
        let errs = Arc::make_mut(&mut lock.errors);
        errs.insert(domain, value);
        ERRORS.set(errs.len() as i64);
        ERRORED.incr(1);
    }
    pub fn clear_error(&self, domain: &'static str) {
        let mut lock = self.lock();
        let errs = Arc::make_mut(&mut lock.errors);
        errs.remove(domain);
        ERRORS.set(errs.len() as i64);
    }
    pub fn update_election(&self, elect: ElectionState) {
        let mut guard = self.lock();
        if !elect.is_leader {
            guard.actions.clear();
            guard.owned_schedule = None;
            if guard.last_scheduler_debug_info.is_some() {
                guard.last_scheduler_debug_info = Arc::new(None);
            }
            let errors = Arc::make_mut(&mut guard.errors);
            errors.remove("reload_configs");
            errors.remove("scheduler_load");
            errors.remove("scheduler");
        }
        let dest_elect = Arc::make_mut(&mut guard.election);
        let tstamp = elect.last_stable_timestamp
            .or(dest_elect.last_stable_timestamp);
        *dest_elect = elect;
        dest_elect.last_stable_timestamp = tstamp;
    }
    pub fn set_parents(&self, parents: Vec<Arc<Schedule>>) {
        self.lock().parent_schedules = Some(parents);
    }
    // Utility
    pub fn leader_cookie(&self) -> Option<LeaderCookie> {
        let mut guard = self.lock();
        if !guard.election.is_leader ||
           *self.fetch_state.get() != fetch::PublicState::StableLeader
        {
            return None;
        }
        return Some(LeaderCookie {
            epoch: guard.election.epoch,
            parent_schedules: guard.parent_schedules.take()
              .unwrap_or(guard.stable_schedule.iter().cloned().collect()),
            actions: guard.actions.clone(),
        })
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
    pub fn read_force_render(&self) -> bool {
        self.force_render.swap(false, SeqCst)
    }
    pub fn force_render(&self) {
        self.force_render.store(true, SeqCst);
        if let Some(x) = self.stable_schedule() {
            self.0.schedule_channel.swap(x)
                .expect("apply channel is alive");
        }
    }
    pub fn push_action(&self, data: Json) -> Result<u64, PushActionError> {
        let mut guard = self.lock();

        if !guard.election.is_leader {
            return Err(PushActionError::NotALeader);
        }
        ACTIONS_EXECUTED.incr(1);

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
            FAILING_ROLES.set(role_errors.len() as i64);
        }
        FAILED_ROLES.incr(1);
    }
    pub fn reset_role_failure(&self, role_name: &str) {
        let mut lock = self.lock();
        let role_errors = Arc::make_mut(&mut lock.failed_roles);
        role_errors.remove(role_name);
        FAILING_ROLES.set(role_errors.len() as i64);
    }
}

pub fn metrics() -> List {
    let global = "global";
    let peers = "peers";
    return vec![
        (Metric(global, "actions_executed"), &*ACTIONS_EXECUTED),
        (Metric(global, "errors_occured"), &*ERRORED),
        (Metric(global, "roles_failed"), &*FAILED_ROLES),
        (Metric(global, "current_errors"), &*ERRORS),
        (Metric(global, "current_failing"), &*FAILING_ROLES),
        (Metric(peers, "known"), &*PEERS),
    ]
}
