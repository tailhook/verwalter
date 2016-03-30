use std::collections::{HashSet, HashMap};
use shared::{Id};
use super::Schedule;


#[derive(Clone, Debug, RustcEncodable)]
pub struct PrefetchInfo {
    /// Can download the following hashes from the following peers
    pub to_fetch: HashMap<(u64, String), Vec<Id>>,

    /// These peers haven't declared their last known schedule yet
    pub peers_left: HashSet<Id>,

    /// We keep track of peers that reported their status too, because
    /// updating the list of peers should work well too
    pub peers_reported: HashSet<Id>,

    /// All different schedules that must be taken into account
    ///
    /// While gathering data we start with either nothing in working_set or
    /// only local schedule in the working set. As we download them and
    /// find them as non-reachable from current working set we add schedules.
    ///
    /// Also we remove things from workingset if they are reachable from the
    /// the schedules which we are going to add.
    ///
    /// Then either to_fetch and peers_left are empty or if a timeout passes
    /// we send all the schedules in the working set to the scheduler
    /// algorithm
    pub working_set: HashMap<(u64, String), Schedule>,

    /// All hashes that can be reached from working set
    pub all_hashes: HashSet<(u64, String)>,
}

impl PrefetchInfo {
    pub fn new<I: Iterator<Item=Id>>(peers: I) -> PrefetchInfo {
        PrefetchInfo {
            all_hashes: HashSet::new(),
            to_fetch: HashMap::new(),
            peers_left: peers.collect(),
            peers_reported: HashSet::new(),
            working_set: HashMap::new(),
        }
    }
    /// Called when we receive report from peer
    ///
    /// Return true if new schedule is added to a "to_fetch" list
    /// (it barely means
    pub fn peer_report(&mut self, id: Id, schedule: (u64, String)) -> bool {
        self.peers_left.remove(&id);
        self.peers_reported.insert(id.clone());
        if self.all_hashes.contains(&schedule) {
            return false;
        }
        self.to_fetch.entry(schedule).or_insert_with(Vec::new)
            .push(id);
        println!("----- TO FETCH {:?} -----", self.to_fetch);
        return true;
    }
}
