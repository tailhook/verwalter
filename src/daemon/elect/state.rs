// use rotor::{GenericScope};

use id::Id;
use elect::Epoch;
use elect::machine::Machine;
use time_util::ToMsec;

/// This is same as elect::machine::Machine, but for easier publishing to
/// API
#[derive(Clone, RustcEncodable, Debug, Default)]
pub struct ElectionState {
    /// Is current node a leader
    pub is_leader: bool,
    /// Is there a leader in a (visible) cluster
    pub is_stable: bool,
    /// A leader if there is one, only if we are not a leader
    pub leader: Option<Id>,
    /// A peer we are promoting if there is no leader and we are not electing
    pub promoting: Option<Id>,
    /// Number of votes for this node to become a leader if it's electing
    pub num_votes_for_me: Option<usize>,
    /// Current epoch (for debugging)
    pub epoch: Epoch,
    /// Current timeout (for debugging), JSON-friendly, in seconds
    pub deadline: u64,
    /// Last known timestamp when cluster was known to be stable
    /// the `ElectionState::from` timestamp returns it either None or now
    /// depending on whether cluster is table. And `shared` module keeps track
    /// of the last one
    pub last_stable_timestamp: Option<u64>,
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
                deadline: scope.estimate_timespec(leader_deadline).to_msec(),
                last_stable_timestamp: None,
            },
            Electing { epoch, ref votes_for_me, deadline } => ElectionState {
                is_leader: false,
                is_stable: false,
                leader: None,
                promoting: None,
                num_votes_for_me: Some(votes_for_me.len()),
                epoch: epoch,
                deadline: scope.estimate_timespec(deadline).to_msec(),
                last_stable_timestamp: None,
            },
            Voted { epoch, ref peer, election_deadline } => ElectionState {
                is_leader: false,
                is_stable: false,
                leader: None,
                promoting: Some(peer.clone()),
                num_votes_for_me: None,
                epoch: epoch,
                deadline: scope.estimate_timespec(election_deadline).to_msec(),
                last_stable_timestamp: None,
            },
            Leader { epoch, next_ping_time } => ElectionState {
                is_leader: true,
                is_stable: true,
                leader: None,
                promoting: None,
                num_votes_for_me: None,
                epoch: epoch,
                deadline: scope.estimate_timespec(next_ping_time).to_msec(),
                last_stable_timestamp:
                    Some(scope.estimate_timespec(scope.now()).to_msec()),
            },
            Follower { ref leader, epoch, leader_deadline } => ElectionState {
                is_leader: false,
                is_stable: true,
                leader: Some(leader.clone()),
                promoting: None,
                num_votes_for_me: None,
                epoch: epoch,
                deadline: scope.estimate_timespec(leader_deadline).to_msec(),
                last_stable_timestamp:
                    Some(scope.estimate_timespec(scope.now()).to_msec()),
            },
        }
    }
}
