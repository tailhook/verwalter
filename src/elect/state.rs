use time::{Timespec};
use rotor::{GenericScope};

use shared::Id;
use elect::Epoch;
use elect::machine::Machine;

/// This is same as elect::machine::Machine, but for easier publishing to
/// API
#[derive(Clone, RustcEncodable, Debug, Default)]
pub struct ElectionState {
    /// Is current node a leader
    is_leader: bool,
    /// Is there a leader in a (visible) cluster
    is_stable: bool,
    /// A leader if there is one, only if we are not a leader
    leader: Option<Id>,
    /// A peer we are promoting if there is no leader and we are not electing
    promoting: Option<Id>,
    /// Number of votes for this node to become a leader if it's electing
    num_votes_for_me: Option<usize>,
    /// Current epoch (for debugging)
    epoch: Epoch,
    /// Current timeout (for debugging), JSON-friendly, in seconds
    deadline: f64,
}

fn to_float(time: Timespec) -> f64 {
    return time.sec as f64 + time.nsec as f64 / 1_000_000_000.0;
}

impl ElectionState {
    pub fn from<S: GenericScope>(src: &Machine, scope: &mut S)
        -> ElectionState
    {
        use elect::machine::Machine::*;
        match *src {
            Starting { leader_deadline } => ElectionState {
                is_leader: false,
                is_stable: false,
                leader: None,
                promoting: None,
                num_votes_for_me: None,
                epoch: 0,
                deadline: to_float(scope.estimate_timespec(leader_deadline)),
            },
            Electing { epoch, ref votes_for_me, deadline } => ElectionState {
                is_leader: false,
                is_stable: false,
                leader: None,
                promoting: None,
                num_votes_for_me: Some(votes_for_me.len()),
                epoch: epoch,
                deadline: to_float(scope.estimate_timespec(deadline)),
            },
            Voted { epoch, ref peer, election_deadline } => ElectionState {
                is_leader: false,
                is_stable: false,
                leader: None,
                promoting: Some(peer.clone()),
                num_votes_for_me: None,
                epoch: epoch,
                deadline: to_float(scope.estimate_timespec(election_deadline)),
            },
            Leader { epoch, next_ping_time } => ElectionState {
                is_leader: true,
                is_stable: true,
                leader: None,
                promoting: None,
                num_votes_for_me: None,
                epoch: epoch,
                deadline: to_float(scope.estimate_timespec(next_ping_time)),
            },
            Follower { ref leader, epoch, leader_deadline } => ElectionState {
                is_leader: false,
                is_stable: true,
                leader: Some(leader.clone()),
                promoting: None,
                num_votes_for_me: None,
                epoch: epoch,
                deadline: to_float(scope.estimate_timespec(leader_deadline)),
            },
        }
    }
}
