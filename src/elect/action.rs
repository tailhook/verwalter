use rotor::Time;

use shared::Id;


#[derive(PartialEq, Eq, Debug)]
pub enum Action {
    PingAll,
    Vote(Id),
    ConfirmVote(Id),
    Pong(Id),
    PingNew,
}

#[derive(PartialEq, Eq, Debug)]
pub struct ActionList {
    pub next_wakeup: Time,
    pub update_schedule: Option<String>,
    pub action: Option<Action>,
}
impl ActionList {
    pub fn and_update(mut self, hash: String) -> ActionList {
        self.update_schedule = Some(hash);
        self
    }
}

impl Action {
    pub fn and_wait(self, time: Time) -> ActionList {
        ActionList {
            next_wakeup: time,
            update_schedule: None,
            action: Some(self),
        }
    }
    pub fn wait(time: Time) -> ActionList {
        ActionList {
            next_wakeup: time,
            update_schedule: None,
            action: None,
        }
    }
}

