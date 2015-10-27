use std::collections::HashSet;

use rand::{thread_rng, Rng};
use time::{SteadyTime, Duration};

use super::{Id, Node, Machine, Message, ExternalData, Capsule};
use super::settings::{start_timeout, election_ivl, HEARTBEAT_INTERVAL};
use super::action::{Action, ActionList};


impl Node {
    pub fn new<S:AsRef<str>>(id: S, now: SteadyTime) -> Node {
        Node {
            id: Id(id.as_ref().to_string()),
            machine: Machine::Starting {
                leader_deadline: now + start_timeout(),
            },
            ext: ExternalData::empty(),
        }
    }
    pub fn time_passed(self, now: SteadyTime) -> (Node, ActionList) {
        use super::Machine::*;
        let (machine, action) = match self.machine {
            Starting { leader_deadline } if leader_deadline <= now => {
                info!("[{}] Time passed. Electing as a leader", self.id);
                if self.ext.all_hosts.len() == 0 {
                    // No other hosts. May safefully become a leader
                    let next_ping = now +
                        Duration::milliseconds(HEARTBEAT_INTERVAL);
                    (Leader { ping_time: next_ping },
                     Action::PingAll.and_wait(next_ping))
                } else {
                    let election_end = now +
                        Duration::milliseconds(HEARTBEAT_INTERVAL);
                    (Electing {
                        votes_for_me: {
                            let mut h = HashSet::new();
                            h.insert(self.id.clone());
                            h },
                        election_deadline: election_end },
                     Action::Vote.and_wait(election_end))
                }
            }
            Starting { leader_deadline: dline }
            => (Starting { leader_deadline: dline }, Action::wait(dline)),
            _ => unimplemented!(),
        };
        return (
            Node {
                machine: machine,
                id: self.id,
                ext: self.ext,
            },
            action)
    }
    pub fn message(self, msg: Capsule, now: SteadyTime) -> (Node, ActionList) {
        use super::Machine::*;
        use super::Message::*;
        let (num, data) = msg;

        let (machine, action) = match (self.machine, data) {
            (Starting { .. }, Ping) => {
                let dline = now + election_ivl();
                (Follower { leader_deadline: dline }, Action::wait(dline))
            }
            (Starting { leader_deadline: dline }, Pong) => {
                // This probably means this node was a leader. But there is
                // no guarantee that no leader has been already elected, so
                // we just continue work
                (Starting { leader_deadline: dline }, Action::wait(dline))
            }
            (Starting { leader_deadline }, Vote(id)) => {
                let dline = now + election_ivl();
                (Voted { peer: id.clone(), election_deadline: dline},
                 Action::ConfirmVote(id).and_wait(leader_deadline))
            }
            (Electing { .. }, _) => unimplemented!(),
            (Voted { .. }, _) => unimplemented!(),
            (Leader { .. }, _) => unimplemented!(),
            (Follower { .. }, _) => unimplemented!(),
        };
        return (
            Node {
                machine: machine,
                id: self.id,
                ext: self.ext,
            },
            action)
    }
}
