use std::net::SocketAddr;
use std::collections::{HashMap};

use rotor::Time;
use rotor::mio::udp::UdpSocket;
use rotor_cantal::Schedule;

pub use self::settings::peers_refresh;
use shared::{Id, Peer, SharedState};

mod machine;
mod action;
mod settings;
mod info;
mod network;
mod encode;
#[cfg(test)] mod test_node;
#[cfg(test)] mod test_mesh;
#[cfg(test)] mod test_util;
#[cfg(test)] mod test_split_brain;

pub struct Election {
    id: Id,
    addr: SocketAddr,
    hostname: String,
    state: SharedState,
    machine: machine::Machine,
    schedule: Schedule,
    socket: UdpSocket,
}

pub type Capsule = (Id, machine::Epoch, Message);

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
    hosts_timestamp: Option<Time>,
    /// State machine of the leader election
    all_hosts: &'a HashMap<Id, Peer>,
}
