use std::collections::{HashSet, HashMap};
use shared::{Id};
use super::Schedule;


#[derive(Clone, Debug, RustcEncodable)]
pub struct PrefetchInfo {
    /// All hashes that can be reached from working set
    pub all_hashes: HashSet<(u64, String)>,
    /// Can download the following hashes from the following peers
    pub todo: HashMap<(u64, String), Vec<Id>>,
    /// All different schedules that must be taken into account
    ///
    /// While gathering data we start with either nothing in working_set or
    /// only local schedule in the working set. As we download them and
    /// find them as non-reachable from current working set we add schedules.
    ///
    /// Also we remove things from workingset if they are reachable from the
    /// the schedules which we are going to add.
    ///
    /// Then either todo is empty or if a timeout passes we send all the
    /// schedules in the working set to the scheduler algorithm
    pub working_set: HashMap<(u64, String), Schedule>,
}

impl PrefetchInfo {
    pub fn new() -> PrefetchInfo {
        PrefetchInfo {
            all_hashes: HashSet::new(),
            todo: HashMap::new(),
            working_set: HashMap::new(),
        }
    }
}
