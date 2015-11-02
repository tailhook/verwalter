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
    Electing { epoch: Epoch, votes_for_me: HashSet<Id>, deadline: SteadyTime },
    Voted { epoch: Epoch, peer: Id, election_deadline: SteadyTime },
    Leader { epoch: Epoch, next_ping_time: SteadyTime },
    Follower { epoch: Epoch, leader_deadline: SteadyTime },
}


impl Machine {
    pub fn new(now: SteadyTime) -> Machine {
        Machine::Starting {
            leader_deadline: now + start_timeout(),
        }
    }

    // methods generic over the all states
    pub fn is_newer_than(&self, epoch: Epoch) -> bool {
        use self::Machine::*;
        let my_epoch = match *self {
            Starting { .. } => 0,  // real epochs start from 1
            Electing { epoch, .. } => epoch,
            Voted { epoch, ..} => epoch,
            Leader { epoch, ..} => epoch,
            Follower { epoch, ..} => epoch,
        };
        my_epoch > epoch
    }
    pub fn current_deadline(&self) -> SteadyTime {
        use self::Machine::*;
        match *self {
            Starting { leader_deadline } => leader_deadline,
            Electing { deadline, .. } => deadline,
            Voted { election_deadline, ..} => election_deadline,
            Leader { next_ping_time, ..} => next_ping_time,
            Follower { leader_deadline, ..} => leader_deadline,
        }
    }

    pub fn time_passed(self, info: &Info, now: SteadyTime)
        -> (Machine, ActionList)
    {
        use self::Machine::*;

        // In case of spurious time events
        if self.current_deadline() > now {
            return pass(self)
        }

        // Everything here assumes that deadline is definitely already passed
        let (machine, action) = match self {
            Starting { .. } => {
                info!("[{}] Time passed. Electing as a leader", info.id);
                if info.all_hosts.len() == 0 {
                    // No other hosts. May safefully become a leader
                    become_leader(1, now)
                } else {
                    start_election(1, now, &info.id)
                }
            }
            Electing { epoch, .. } => {
                // It's decided that even if at the end of election we
                // suddenly have >= minimum votes it's safely to start new
                // election.
                //
                // I mean in the following case:
                //
                // 1. Node starts election
                // 2. Node receives few votes
                // 3. Number of peers drop (i.e. some nodes fail)
                // 4. Timeout expires
                //
                // .. we start election again instead of trying to count votes
                // again (e.g. if failed nodes are voted for the node)
                info!("[{}] Time passed. Starting new election", info.id);
                start_election(epoch+1, now, &info.id)
            },
            Voted { epoch, .. } => {
                info!("[{}] Time passed. Elect me please", info.id);
                start_election(epoch+1, now, &info.id)
            }
            me @ Leader { .. } => {
                let next_ping = now +
                    Duration::milliseconds(HEARTBEAT_INTERVAL);
                (me,
                 Action::PingAll.and_wait(next_ping))
            }
            Follower { epoch, .. } => {
                info!("[{}] Leader is unresponsive. Elect me please", info.id);
                start_election(epoch+1, now, &info.id)
            }
        };
        return (machine, action)
    }
    pub fn message(self, info: &Info, msg: Capsule, now: SteadyTime)
        -> (Machine, ActionList)
    {
        use self::Machine::*;
        use super::Message::*;
        let (src, msg_epoch, data) = msg;
        if self.is_newer_than(msg_epoch) {
            return pass(self);
        }

        let (machine, action) = match (self, data) {
            (Starting { .. }, Ping) => {
                // Got message that someone is a leader
                follow(msg_epoch, now)
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
            (Electing { .. }, Ping) => {
                // Got message that someone is already (just became) a leader
                follow(msg_epoch, now)
            }
            (me @ Electing { .. }, Pong) => {
                // This is not expected when the network is okay and all nodes
                // behave well because it means someone thinks that this node
                // is a leader
                pass(me)
            }
            (Electing { epoch, mut votes_for_me, deadline}, Vote(id)) => {
                if id == info.id {
                    votes_for_me.insert(src);
                    let need = minimum_votes(info.all_hosts.len());
                    if votes_for_me.len() >= need {
                        become_leader(epoch, now)
                    } else {
                        (Electing { epoch: epoch, votes_for_me: votes_for_me,
                                    deadline: deadline },
                         Action::wait(deadline))
                    }
                } else {
                    // Peer voted for someone else
                    (Electing { epoch: epoch, votes_for_me: votes_for_me,
                                deadline: deadline },
                     Action::wait(deadline))
                }
            }
            (Voted { .. }, _) => unimplemented!(),
            (Leader { .. }, _) => unimplemented!(),
            (Follower { .. }, _) => unimplemented!(),
        };
        return (machine, action)
    }
}

fn follow(epoch: Epoch, now: SteadyTime) -> (Machine, ActionList) {
    let dline = now + election_ivl();
    (Machine::Follower { epoch: epoch, leader_deadline: dline },
     Action::Pong.and_wait(dline))
}

fn pass(me: Machine) -> (Machine, ActionList) {
    let deadline = me.current_deadline();
    return (me, Action::wait(deadline));
}

fn minimum_votes(total_peers: usize) -> usize {
    match total_peers + 1 {  // peers don't include myself
        0 => 0,
        1 => 1,
        2 => 2,
        x => (x >> 1) + 1,
    }
}

fn become_leader(epoch: Epoch, now: SteadyTime) -> (Machine, ActionList) {
    let next_ping = now +
        Duration::milliseconds(HEARTBEAT_INTERVAL);
    (Machine::Leader { epoch: epoch, next_ping_time: next_ping },
     Action::PingAll.and_wait(next_ping))
}

fn start_election(epoch: Epoch, now: SteadyTime, first_vote: &Id)
    -> (Machine, ActionList)
{
    let election_end = now +
        Duration::milliseconds(HEARTBEAT_INTERVAL);
    (Machine::Electing {
        epoch: epoch,
        votes_for_me: {
            let mut h = HashSet::new();
            h.insert(first_vote.clone());
            h },
        deadline: election_end },
     Action::Vote(first_vote.clone()).and_wait(election_end))
}
