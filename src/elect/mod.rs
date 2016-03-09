use std::net::SocketAddr;
use std::sync::{Arc};
use std::collections::{HashSet, HashMap};
use rustc_serialize::hex::ToHex;

use rotor_cantal::Schedule;
use time::Timespec;

mod machine;
mod action;
mod settings;
mod info;
mod network;
#[cfg(test)] mod test_node;
#[cfg(test)] mod test_mesh;
#[cfg(test)] mod test_util;
#[cfg(test)] mod test_split_brain;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Id(Box<[u8]>);

pub struct Election {
    info: Info,
    machine: machine::Machine,
    schedule: Schedule,
}

type Capsule = (Id, u64, Message);

#[derive(Clone, Debug)]
enum Message {
    /// Ping message from leader to followers, reassures that leadership
    /// still holds
    Ping,
    /// Pong message from follower to leader, confirm that node is a leader
    Pong,
    /// Vote for some node
    Vote(Id),
}

#[derive(Clone, Debug)]
struct PeerInfo {
     addr: Option<SocketAddr>,
     last_report: Option<Timespec>,
}

#[derive(Debug)]
struct Info {
    /// Unique identificator of the node, should be read from /etc/machine-id
    id: Id,
    /// State machine of the leader election
    all_hosts: HashMap<Id, PeerInfo>,
}

impl ::std::fmt::Display for Id {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(fmt, "{}", self.0.to_hex())
    }
}
