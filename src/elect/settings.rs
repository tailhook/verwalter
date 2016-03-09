use std::time::Duration;

use rand::{thread_rng, Rng};

/// The number of seconds just started worker waits it to be reached by
/// leader. We usually don't care joining the cluster on start too much.
/// Probably we want it to be bigger than:
///
/// * TCP retransmision timeout (so that in flaky network we don't start too
///   much elections, because they have big chance to fail)
/// * Cantal's discovery time and time to propagate changes to the leader
///
/// The random MESSAGE_TIMEOUT (constained by constants below) is added to
/// this timeout.
pub const START_TIMEOUT: u64 = 5000;

/// On each leader's ping we start election timer of a random value in this
/// range. If there is no heartbeat from leader during this timeout, we start
/// election. Note that mio currently has only 200 ms precision timers.
pub const MIN_MESSAGE_TIMEOUT: u64 = 1200;
pub const MAX_MESSAGE_TIMEOUT: u64 = 3000;

/// Leader ping interval
///
/// Raft have it slightly less than MIN_MESSAGE_TIMEOUT. My intuition says that
/// it's nicer to have 2x smaller, just like in almost every other heartbeating
/// system. There is no good reason to wait so long for original Raft. I.e.
/// it wants to reestablish consistency as fast as possible. But it may be
/// nicer to keep lower elections for us.
pub const HEARTBEAT_INTERVAL: u64 = 600;


pub fn start_timeout() -> Duration {
    Duration::from_millis(START_TIMEOUT) + election_ivl()
}

pub fn election_ivl() -> Duration {
    Duration::from_millis(
        thread_rng().gen_range(MIN_MESSAGE_TIMEOUT, MAX_MESSAGE_TIMEOUT))
}
