use std::collections::HashSet;
use std::cmp::{Ord, Ordering};
use std::cmp::Ordering::{Less as Older, Equal as Current, Greater as Newer};
use std::time::{Duration, SystemTime};

use rotor::Time;
use libcantal::{Counter, Integer};

use time_util::ToMsec;
use shared::Id;
use super::{Info, Message};
use super::settings::{start_timeout, election_ivl, HEARTBEAT_INTERVAL};
use super::action::{Action, ActionList};

pub type Epoch = u64;

lazy_static! {
    pub static ref START_ELECTION_NO: Counter = Counter::new();
    pub static ref START_ELECTION_TM: Integer = Integer::new();
    pub static ref PING_ALL_NO: Counter = Counter::new();
    pub static ref PING_ALL_TM: Integer = Integer::new();
    pub static ref OUTDATED_NO: Counter = Counter::new();
    pub static ref OUTDATED_TM: Integer = Integer::new();
    pub static ref PING_NO: Counter = Counter::new();
    pub static ref PING_TM: Integer = Integer::new();
    pub static ref PONG_NO: Counter = Counter::new();
    pub static ref PONG_TM: Integer = Integer::new();
    pub static ref VOTE_CONFIRM_NO: Counter = Counter::new();
    pub static ref VOTE_CONFIRM_TM: Integer = Integer::new();
    pub static ref BECAME_LEADER_NO: Counter = Counter::new();
    pub static ref BECAME_LEADER_TM: Integer = Integer::new();
    pub static ref VOTE_FOR_ME_NO: Counter = Counter::new();
    pub static ref VOTE_FOR_ME_TM: Integer = Integer::new();
    pub static ref VOTE_OTHER_NO: Counter = Counter::new();
    pub static ref VOTE_OTHER_TM: Integer = Integer::new();
    pub static ref LATE_VOTE_NO: Counter = Counter::new();
    pub static ref LATE_VOTE_TM: Integer = Integer::new();
    pub static ref NEWER_PING_NO: Counter = Counter::new();
    pub static ref NEWER_PING_TM: Integer = Integer::new();
    pub static ref NEW_VOTE_NO: Counter = Counter::new();
    pub static ref NEW_VOTE_TM: Integer = Integer::new();
    pub static ref BAD_HOSTS_NO: Counter = Counter::new();
    pub static ref BAD_HOSTS_TM: Integer = Integer::new();
    pub static ref SELF_ELECT_NO: Counter = Counter::new();
    pub static ref SELF_ELECT_TM: Integer = Integer::new();
}
lazy_static! {
    // Election reasons
    pub static ref ELECT_START_NO: Counter = Counter::new();
    pub static ref ELECT_START_TM: Integer = Integer::new();
    pub static ref ELECT_TIMEO_NO: Counter = Counter::new();
    pub static ref ELECT_TIMEO_TM: Integer = Integer::new();
    pub static ref ELECT_VOTED_NO: Counter = Counter::new();
    pub static ref ELECT_VOTED_TM: Integer = Integer::new();
    pub static ref ELECT_UNRESPONSIVE_NO: Counter = Counter::new();
    pub static ref ELECT_UNRESPONSIVE_TM: Integer = Integer::new();
    pub static ref ELECT_CONFLICT_NO: Counter = Counter::new();
    pub static ref ELECT_CONFLICT_TM: Integer = Integer::new();
    pub static ref ELECT_UNSOLICIT_PONG_NO: Counter = Counter::new();
    pub static ref ELECT_UNSOLICIT_PONG_TM: Integer = Integer::new();
    pub static ref ELECT_NEWER_PONG_NO: Counter = Counter::new();
    pub static ref ELECT_NEWER_PONG_TM: Integer = Integer::new();
}

#[derive(Clone, Debug)]
pub enum Machine {
    Starting { leader_deadline: Time },
    Electing { epoch: Epoch, votes_for_me: HashSet<Id>, deadline: Time },
    Voted { epoch: Epoch, peer: Id, election_deadline: Time },
    Leader { epoch: Epoch, next_ping_time: Time },
    Follower { leader: Id, epoch: Epoch, leader_deadline: Time },
}

fn report(cnt: &Counter, time: &Integer) {
    cnt.incr(1);
    time.set(SystemTime::now().to_msec() as i64);
}

impl Machine {
    pub fn new(now: Time) -> Machine {
        Machine::Starting {
            leader_deadline: now + start_timeout(),
        }
    }

    /// This method should only be used for the external messages
    /// and for compare_epoch. Don't use it in the code directly
    pub fn current_epoch(&self) -> u64 {
        use self::Machine::*;
        match *self {
            Starting { .. } => 0,  // real epochs start from 1
            Electing { epoch, .. } => epoch,
            Voted { epoch, ..} => epoch,
            Leader { epoch, ..} => epoch,
            Follower { epoch, ..} => epoch,
        }
    }

    // methods generic over the all states
    pub fn compare_epoch(&self, epoch: Epoch) -> Ordering {
        let my_epoch = self.current_epoch();
        epoch.cmp(&my_epoch)
    }
    pub fn current_deadline(&self) -> Time {
        use self::Machine::*;
        match *self {
            Starting { leader_deadline } => leader_deadline,
            Electing { deadline, .. } => deadline,
            Voted { election_deadline, ..} => election_deadline,
            Leader { next_ping_time, ..} => next_ping_time,
            Follower { leader_deadline, ..} => leader_deadline,
        }
    }

    pub fn time_passed(self, info: &Info, now: Time)
        -> (Machine, ActionList)
    {
        use self::Machine::*;

        debug!("[{}] time {:?} passed (me: {:?})",
            self.current_epoch(), self.current_deadline(), self);
        // In case of spurious time events
        if self.current_deadline() > now {
            return pass(self)
        }

        // We can't do much useful work if our peers info is outdated
        if !info.hosts_are_fresh() {
            // TODO(tailhook) We have to give up our leadership though
            report(&BAD_HOSTS_NO, &BAD_HOSTS_TM);
            return pass(self)
        }

        // Everything here assumes that deadline is definitely already passed
        let (machine, action) = match self {
            Starting { .. } => {
                info!("[{}] Time passed. Electing as a leader", info.id);
                if info.all_hosts.len() == 1 || info.debug_force_leader {
                    // No other hosts. May safefully become a leader
                    report(&SELF_ELECT_NO, &SELF_ELECT_TM);
                    become_leader(1, now)
                } else {
                    report(&ELECT_START_NO, &ELECT_START_TM);
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
                report(&ELECT_TIMEO_NO, &ELECT_TIMEO_TM);
                start_election(epoch+1, now, &info.id)
            },
            Voted { epoch, .. } => {
                info!("[{}] Time passed. Elect me please", info.id);
                report(&ELECT_VOTED_NO, &ELECT_VOTED_TM);
                start_election(epoch+1, now, &info.id)
            }
            Leader { epoch, .. } => {
                // TODO(tailhook) see if we have slept just too much
                //                give up leadership right now
                let next_ping = now +
                    Duration::from_millis(HEARTBEAT_INTERVAL);
                report(&PING_ALL_NO, &PING_ALL_TM);
                (Leader { epoch: epoch, next_ping_time: next_ping },
                 Action::PingAll.and_wait(next_ping))
            }
            Follower { epoch, .. } => {
                info!("[{}] Leader is unresponsive. Elect me please", info.id);
                report(&ELECT_UNRESPONSIVE_NO, &ELECT_UNRESPONSIVE_TM);
                start_election(epoch+1, now, &info.id)
            }
        };
        return (machine, action)
    }
    pub fn message(self, info: &Info, msg: (Id, Epoch, Message), now: Time)
        -> (Machine, ActionList)
    {
        use self::Machine::*;
        use super::Message::*;
        let (src, msg_epoch, data) = msg;
        let epoch_cmp = self.compare_epoch(msg_epoch);
        debug!("[{}] Message {:?} from {} with epoch {:?} (me: {:?})",
            self.current_epoch(), data, src, msg_epoch, self);
        let (machine, action) = match (data, epoch_cmp, self) {
            (_, Older, me) => { // discard old messages
                report(&OUTDATED_NO, &OUTDATED_TM);
                pass(me)
            }
            (Ping, Current, Leader { .. }) => {
                // Another leader is here, restart the election
                // This is valid when two partitions suddenly joined
                report(&ELECT_CONFLICT_NO, &ELECT_CONFLICT_TM);
                start_election(msg_epoch+1, now, &info.id)
            }
            (Ping, Current, _) => {
                // Ping in any other state, means we follow the leader
                report(&PING_NO, &PING_TM);
                follow(src, msg_epoch, now)
            }
            (Pong, Current, me @ Leader { .. }) => {
                report(&PONG_NO, &PONG_TM);
                pass(me)
            }
            (Pong, Current, _) => {
                // Pong in any other state means something wrong with other
                // peers thinking of who is a leader
                report(&ELECT_UNSOLICIT_PONG_NO, &ELECT_UNSOLICIT_PONG_TM);
                start_election(msg_epoch+1, now, &info.id)
            }
            (Vote(id), Current, Starting { .. }) => {
                let dline = now + election_ivl();
                report(&VOTE_CONFIRM_NO, &VOTE_CONFIRM_TM);
                (Voted { epoch: msg_epoch,
                    peer: id.clone(), election_deadline: dline},
                 Action::ConfirmVote(id).and_wait(dline))
            }
            (Vote(id), Current, Electing {epoch, mut votes_for_me, deadline})
            => {
                if id == *info.id {
                    report(&VOTE_FOR_ME_NO, &VOTE_FOR_ME_TM);
                    votes_for_me.insert(src);
                    let need = minimum_votes(info.all_hosts.len());
                    if votes_for_me.len() >= need {
                        report(&BECAME_LEADER_NO, &BECAME_LEADER_TM);
                        become_leader(epoch, now)
                    } else {
                        (Electing { epoch: epoch, votes_for_me: votes_for_me,
                                    deadline: deadline },
                         Action::wait(deadline))
                    }
                } else {
                    report(&VOTE_OTHER_NO, &VOTE_OTHER_TM);
                    // Peer voted for someone else
                    (Electing { epoch: epoch, votes_for_me: votes_for_me,
                                deadline: deadline },
                     Action::wait(deadline))
                }
            }
            (Vote(_), Current, me @ Voted { .. })
            | (Vote(_), Current, me @ Leader { .. })
            | (Vote(_), Current, me @ Follower { .. })
            => {
                // This vote is late for the party
                report(&LATE_VOTE_NO, &LATE_VOTE_TM);
                pass(me)
            }
            (Ping, Newer, _) => {
                // We missed something, there is already a new leader
                report(&NEWER_PING_NO, &NEWER_PING_TM);
                follow(src, msg_epoch, now)
            }
            (Pong, Newer, _) => {
                // Something terribly wrong: somebody thinks that we are leader
                // in the new epoch. Just start a new election
                report(&ELECT_NEWER_PONG_NO, &ELECT_NEWER_PONG_TM);
                start_election(msg_epoch+1, now, &info.id)
            }
            (Vote(id), Newer, _) => {
                // Somebody started an election, just trust him
                let dline = now + election_ivl();
                report(&NEW_VOTE_NO, &NEW_VOTE_TM);
                (Voted { epoch: msg_epoch,
                    peer: id.clone(), election_deadline: dline},
                 Action::ConfirmVote(id).and_wait(dline))
            }
        };
        return (machine, action)
    }
}

fn follow(leader: Id, epoch: Epoch, now: Time)
    -> (Machine, ActionList)
{
    let dline = now + election_ivl();
    (Machine::Follower { leader: leader.clone(), epoch: epoch,
                         leader_deadline: dline },
     Action::Pong(leader).and_wait(dline))
}

fn pass(me: Machine) -> (Machine, ActionList) {
    let deadline = me.current_deadline();
    return (me, Action::wait(deadline));
}

fn minimum_votes(total_peers: usize) -> usize {
    match total_peers {  // peers do include myself
        0 => 0,
        1 => 1,
        2 => 2,
        x => (x >> 1) + 1,
    }
}

fn become_leader(epoch: Epoch, now: Time) -> (Machine, ActionList) {
    let next_ping = now + Duration::from_millis(HEARTBEAT_INTERVAL);
    (Machine::Leader { epoch: epoch, next_ping_time: next_ping },
     Action::PingAll.and_wait(next_ping))
}

fn start_election(epoch: Epoch, now: Time, first_vote: &Id)
    -> (Machine, ActionList)
{
    report(&START_ELECTION_NO, &START_ELECTION_TM);
    let election_end = now + Duration::from_millis(HEARTBEAT_INTERVAL);
    (Machine::Electing {
        epoch: epoch,
        votes_for_me: {
            let mut h = HashSet::new();
            h.insert(first_vote.clone());
            h },
        deadline: election_end },
     Action::Vote(first_vote.clone()).and_wait(election_end))
}
