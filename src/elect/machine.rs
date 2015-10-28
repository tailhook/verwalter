use std::collections::HashSet;

use rand::{thread_rng, Rng};
use time::{SteadyTime, Duration};

use super::{Id, Message, Info, Capsule};
use super::settings::{start_timeout, election_ivl, HEARTBEAT_INTERVAL};
use super::action::{Action, ActionList};


type Epoch = u64;


#[derive(Clone, Debug)]
pub enum Machine {
    Starting { leader_deadline: SteadyTime },
    Electing { epoch: Epoch,
        votes_for_me: HashSet<Id>, election_deadline: SteadyTime },
    Voted { epoch: Epoch, peer: Id, election_deadline: SteadyTime },
    Leader { epoch: Epoch, ping_time: SteadyTime },
    Follower { epoch: Epoch, leader_deadline: SteadyTime },
}


impl Machine {
    pub fn new(now: SteadyTime) -> Machine {
        Machine::Starting {
            leader_deadline: now + start_timeout(),
        }
    }
    pub fn time_passed(self, info: &Info, now: SteadyTime)
        -> (Machine, ActionList)
    {
        use self::Machine::*;
        let (machine, action) = match self {
            Starting { leader_deadline } if leader_deadline <= now => {
                info!("[{}] Time passed. Electing as a leader", info.id);
                if info.all_hosts.len() == 0 {
                    // No other hosts. May safefully become a leader
                    let next_ping = now +
                        Duration::milliseconds(HEARTBEAT_INTERVAL);
                    (Leader { epoch: 1, ping_time: next_ping },
                     Action::PingAll.and_wait(next_ping))
                } else {
                    let election_end = now +
                        Duration::milliseconds(HEARTBEAT_INTERVAL);
                    (Electing {
                        epoch: 1,
                        votes_for_me: {
                            let mut h = HashSet::new();
                            h.insert(info.id.clone());
                            h },
                        election_deadline: election_end },
                     Action::Vote.and_wait(election_end))
                }
            }
            Starting { leader_deadline: dline }
            => (Starting { leader_deadline: dline }, Action::wait(dline)),
            _ => unimplemented!(),
        };
        return (machine, action)
    }
    pub fn message(self, info: &Info, msg: Capsule, now: SteadyTime)
        -> (Machine, ActionList)
    {
        use self::Machine::*;
        use super::Message::*;
        let (msg_epoch, data) = msg;

        let (machine, action) = match (self, data) {
            (Starting { .. }, Ping) => {
                let dline = now + election_ivl();
                (Follower { epoch: msg_epoch, leader_deadline: dline },
                 Action::wait(dline))
            }
            (Starting { leader_deadline: dline }, Pong) => {
                // This probably means this node was a leader. But there is
                // no guarantee that no leader has been already elected, so
                // we just continue work
                (Starting { leader_deadline: dline }, Action::wait(dline))
            }
            (Starting { leader_deadline }, Vote(id)) => {
                let dline = now + election_ivl();
                (Voted { epoch: msg_epoch,
                    peer: id.clone(), election_deadline: dline},
                 Action::ConfirmVote(id).and_wait(leader_deadline))
            }
            (Electing { .. }, _) => unimplemented!(),
            (Voted { .. }, _) => unimplemented!(),
            (Leader { .. }, _) => unimplemented!(),
            (Follower { .. }, _) => unimplemented!(),
        };
        return (machine, action)
    }
}
