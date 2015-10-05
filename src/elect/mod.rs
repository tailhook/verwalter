use std::net::SocketAddr;
use std::collections::HashSet;

use time::SteadyTime;

mod node;
mod settings;
#[cfg(test)] mod test_mesh;
#[cfg(test)] mod test_util;
#[cfg(test)] mod test_split_brain;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Id(String);


enum Machine {
    Starting { leader_deadline: SteadyTime },
    Electing { votes_for_me: HashSet<Id>, election_deadline: SteadyTime },
    Voted { peer: Id, election_deadline: SteadyTime },
    Leader { ping_time: SteadyTime },
    Follower { leader_deadline: SteadyTime },
}

struct Node {
    id: String,
    machine: Machine,
}
