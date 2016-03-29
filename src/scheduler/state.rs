use std::sync::{Arc, Mutex};
use std::collections::{HashSet, HashMap};

use rustc_serialize::json::Json;
use rustc_serialize::{Encodable, Encoder};

use shared::Id;


#[derive(Clone, Debug, RustcEncodable)]
pub struct BuildInfo {
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


#[derive(Clone, Debug, RustcEncodable)]
pub struct Schedule {
    pub timestamp: u64,
    pub hash: String,
    pub data: Json,
    pub origin: bool,
}

#[derive(Clone, Debug, RustcEncodable)]
pub enum FollowerState {
    Waiting,
    Fetching(String),
    Stable(Arc<Schedule>),
}

#[derive(Debug)]
pub enum LeaderState {
    Building(Mutex<BuildInfo>),
    Calculating,
    Stable(Arc<Schedule>),
}


#[derive(Debug, RustcEncodable)]
pub enum State {
    Unstable,
    // Follower states
    Following(Id, FollowerState),
    Leading(LeaderState),
}

impl BuildInfo {
    pub fn new() -> BuildInfo {
        BuildInfo {
            all_hashes: HashSet::new(),
            todo: HashMap::new(),
            working_set: HashMap::new(),
        }
    }
}

impl Encodable for LeaderState {
     fn encode<E: Encoder>(&self, e: &mut E) -> Result<(), E::Error> {
        use self::LeaderState::*;
        e.emit_enum("LeaderState", |e| {
            match *self {
                Building(ref x) => {
                    e.emit_enum_variant("Building", 0, 1, |e| {
                        e.emit_enum_variant_arg(0, |e| {
                            x.lock().expect("buildinfo lock").encode(e)
                        })
                    })
                }
                Calculating => {
                    e.emit_enum_variant("Calculating", 1, 0, |_| {Ok(())})
                }
                Stable(ref x) => {
                    e.emit_enum_variant("Stable", 2, 1, |e| {
                        e.emit_enum_variant_arg(0, |e| {
                            x.encode(e)
                        })
                    })
                }
            }
        })
     }
}
