use std::time::SystemTime;
use std::collections::{HashMap};

use crossbeam::atomic::ArcCell;
use serde_millis;

pub use self::machine::Epoch;
pub use self::network::spawn_election;
pub use self::settings::peers_refresh;
pub use self::state::ElectionState;
use id::Id;
use peer::Peer;

mod action;
mod info;
mod state;

pub mod machine;  // pub for making counters visible
pub mod network;  // pub for making counters visible
pub mod settings;

mod encode;
#[cfg(test)] mod test_node;
#[cfg(test)] mod test_mesh;
#[cfg(test)] mod test_util;
#[cfg(test)] mod test_split_brain;


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduleStamp {
    #[serde(with="serde_millis")]
    pub timestamp: SystemTime,
    pub hash: String,
    pub origin: Id,
}

#[derive(Debug)]
pub struct Capsule {
    source: Id,
    epoch: Epoch,
    message: Message,
    schedule: Option<ScheduleStamp>,
    errors: Option<usize>,
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
    all_hosts: &'a HashMap<Id, ArcCell<Peer>>,
    /// Forces this node to be a leader, this is only for debugging purposes
    debug_force_leader: bool,
    /// Allow becoming a leader in minority partition (i.e. if majority nodes
    /// are unavailable) of a split-brain.
    allow_minority: bool,
}
