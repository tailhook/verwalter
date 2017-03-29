use std::time::{Instant, SystemTime, Duration};

use id::Id;


#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    PingAll,
    Vote(Id),
    ConfirmVote(Id),
    Pong(Id),
}

#[derive(PartialEq, Eq, Debug)]
pub struct ActionList {
    pub next_wakeup: Instant,
    pub action: Option<Action>,
}

impl Action {
    pub fn and_wait(self, time: SystemTime) -> ActionList {
        let delay = time.duration_since(SystemTime::now())
            .unwrap_or(Duration::new(0, 0));
        ActionList {
            next_wakeup: Instant::now() + delay,
            action: Some(self),
        }
    }
    pub fn wait(time: SystemTime) -> ActionList {
        let delay = time.duration_since(SystemTime::now())
            .unwrap_or(Duration::new(0, 0));
        ActionList {
            next_wakeup: Instant::now() + delay,
            action: None,
        }
    }
}

