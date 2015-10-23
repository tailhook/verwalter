use time::SteadyTime;


#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    PingAll,
    Vote,
    ConfirmVote,
    PingNew,
}

#[derive(PartialEq, Eq, Debug)]
pub struct ActionList {
    pub next_wakeup: SteadyTime,
    pub action: Option<Action>,
}

impl Action {
    pub fn and_wait(self, time: SteadyTime) -> ActionList {
        ActionList {
            next_wakeup: time,
            action: Some(self),
        }
    }
    pub fn wait(time: SteadyTime) -> ActionList {
        ActionList {
            next_wakeup: time,
            action: None,
        }
    }
}

