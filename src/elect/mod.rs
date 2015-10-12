use std::net::SocketAddr;
use std::collections::{HashSet, HashMap};

use time::SteadyTime;
use time::Timespec;

mod node;
mod action;
mod settings;
mod external;
#[cfg(test)] mod test_node;
#[cfg(test)] mod test_mesh;
#[cfg(test)] mod test_util;
#[cfg(test)] mod test_split_brain;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Id(String);



#[derive(Clone, Debug)]
enum Machine {
    Starting { leader_deadline: SteadyTime },
    Electing { votes_for_me: HashSet<Id>, election_deadline: SteadyTime },
    Voted { peer: Id, election_deadline: SteadyTime },
    Leader { ping_time: SteadyTime },
    Follower { leader_deadline: SteadyTime },
}

#[derive(Clone, Debug)]
struct PeerInfo {
     addr: SocketAddr,
     last_report: Timespec,
}

#[derive(Clone, Debug)]
struct ExternalData {
    all_hosts: HashMap<Id, PeerInfo>,
}

#[derive(Debug)]
struct Node {
    id: String,
    machine: Machine,
    ext: ExternalData,
}
