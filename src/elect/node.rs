use rand::{thread_rng, Rng};
use time::{SteadyTime, Duration};

use super::{Node, Machine};
use super::settings::start_timeout;


impl Node {
    pub fn new<S:AsRef<str>>(id: S, now: SteadyTime) -> Node {
        Node {
            id: id.as_ref().to_string(),
            machine: Machine::Starting {
                leader_deadline: now + start_timeout(),
            }
        }
    }
}
