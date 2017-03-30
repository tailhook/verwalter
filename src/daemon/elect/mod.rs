use std::net::SocketAddr;
use std::time::SystemTime;
use std::collections::{HashMap};


use id::Id;
use peer::Peer;
pub use self::machine::Epoch;
pub use self::settings::peers_refresh;
pub use self::state::ElectionState;
/*
use shared::{Id, Peer, SharedState};
*/

mod action;
mod info;
mod settings;
mod state;

pub mod machine;  // pub for making counters visible

/*
pub mod network;  // pub for making counters visible
mod encode;
#[cfg(test)] mod test_node;
#[cfg(test)] mod test_mesh;
#[cfg(test)] mod test_util;
#[cfg(test)] mod test_split_brain;
*/

pub struct Election {
    id: Id,
    addr: SocketAddr,
    hostname: String,
    name: String,
    //state: SharedState,
    last_schedule_sent: String,
    //machine: machine::Machine,
    //cantal: Cantal,
    //socket: UdpSocket,
    debug_force_leader: bool,
}

#[derive(Debug)]
pub struct ScheduleStamp {
    pub timestamp: u64,
    pub hash: String,
    pub origin: Id,
}

#[derive(Debug)]
pub struct Capsule {
    source: Id,
    epoch: Epoch,
    message: Message,
    schedule: Option<ScheduleStamp>,
}

#[derive(Clone, Debug)]
pub enum Message {
    /// Ping message from leader to followers, reassures that leadership
    /// still holds
    Ping,
    /// Pong message from follower to leader, confirm that node is a leader
    Pong,
    /// Vote for some node
    Vote(Id),
}

#[derive(Debug)]
pub struct Info<'a> {
    /// Unique identificator of the node, should be read from /etc/machine-id
    id: &'a Id,
    /// This is used to find out whether hosts are actually valid
    hosts_timestamp: Option<SystemTime>,
    /// State machine of the leader election
    all_hosts: &'a HashMap<Id, Peer>,
    /// Forces this node to be a leader, this is only for debugging purposes
    debug_force_leader: bool,
}
