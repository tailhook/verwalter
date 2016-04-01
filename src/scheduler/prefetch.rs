use std::sync::Arc;
use std::collections::{HashSet, HashMap};
use std::collections::hash_map::Entry::{Vacant, Occupied};

use time::{SteadyTime, precise_time_ns};
use rustc_serialize::{Encodable, Encoder};

use shared::{Id};
use elect::ScheduleStamp;
use super::{Schedule, Hash};


#[derive(Clone, Debug)]
pub struct Fetching {
    /// A timestamp when we started to download the data at.
    pub time: Option<SteadyTime>,
    /// If downloading from some host is too slow or not started yet, we get
    /// first item from a HashSet and try again. Two hosts sending same
    /// response is fine.
    ///
    /// Note: we rely on HashSet to provide randomized order for our IDs
    pub sources: HashSet<Id>,
}


#[derive(Clone, Debug, RustcEncodable)]
pub struct PrefetchInfo {
    /// This structure holds hashes that needs to be downloaded
    pub fetching: HashMap<Hash, Fetching>,

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
            fetching: HashMap::new(),
            peers_left: peers.collect(),
            peers_reported: HashSet::new(),
            leader_stamps: HashMap::new(),
            all_schedules: HashMap::new(),
        }
    }
    /// Called when we receive report from peer
    ///
    /// Return true if new schedule is added to a "fetching" list
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
            self.fetching.entry(schedule.hash)
                .or_insert_with(Fetching::new)
                .sources.insert(id);
        }
        return updated;
    }
}

impl Fetching {
    pub fn new() -> Fetching {
        Fetching {
            time: None,
            sources: HashSet::new(),
        }
    }
}

impl Encodable for Fetching {
    fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        e.emit_struct("Fetching", 2, |e| {
            try!(e.emit_struct_field("time", 0, |e| {
                // in milliseconds for javascript
                self.time.map(|tm| {
                    (precise_time_ns() / 1_000_000) as i64 -
                        (SteadyTime::now() - tm).num_milliseconds() as i64
                }).encode(e)
            }));
            try!(e.emit_struct_field("sources", 1, |e| {
                self.sources.encode(e)
            }));
            Ok(())
        })
    }
}
