use std::sync::Arc;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::Entry::{Vacant, Occupied};

use shared::{Id};
use elect::ScheduleStamp;

use super::{Schedule, Hash};


#[derive(Clone, Debug, RustcEncodable)]
pub struct PrefetchInfo {
    /// Can download the following hashes from the following peers
    ///
    /// Note: we rely on HashSet to provide randomized order for our IDs
    pub to_fetch: HashMap<Hash, HashSet<Id>>,

    /// These peers haven't declared their last known schedule yet
    pub peers_left: HashSet<Id>,

    /// We keep track of peers that reported their status too, because
    /// updating the list of peers should work well too
    pub peers_reported: HashSet<Id>,

    /// For each leader that was known by any of the peers, find out
    /// latest schedule by a timestamp
    ///
    /// There coundn't be multiple schedules at the same or nearly same
    /// timestamps, so this heuristic should be good enough.
    pub leader_stamps: HashMap<Id, (u64, Hash)>,

    /// Just storage for downloaded schedules. We don't remove anything
    /// from here until actual scheduling is done
    pub all_schedules: HashMap<Hash, Arc<Schedule>>,
}

impl PrefetchInfo {
    pub fn new<I: Iterator<Item=Id>>(peers: I)
        -> PrefetchInfo
    {
        PrefetchInfo {
            to_fetch: HashMap::new(),
            peers_left: peers.collect(),
            peers_reported: HashSet::new(),
            leader_stamps: HashMap::new(),
            all_schedules: HashMap::new(),
        }
    }
    /// Called when we receive report from peer
    ///
    /// Return true if new schedule is added to a "to_fetch" list
    /// (it barely means to wake up a fetcher)
    pub fn peer_report(&mut self, id: Id, schedule: ScheduleStamp) -> bool {
        self.peers_left.remove(&id);
        self.peers_reported.insert(id.clone());
        let updated = match self.leader_stamps.entry(schedule.origin) {
            Vacant(e) => {
                e.insert((schedule.timestamp, schedule.hash.clone()));
                true
            }
            Occupied(ref mut e) if e.get().0 < schedule.timestamp => {
                e.insert((schedule.timestamp, schedule.hash.clone()));
                true
            }
            Occupied(_) => {
                false
            }
        };
        if updated {
            self.to_fetch.entry(schedule.hash).or_insert_with(HashSet::new)
                .insert(id);
        }
        return updated;
    }
}
