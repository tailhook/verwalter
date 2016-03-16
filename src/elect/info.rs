use rotor::Time;

use super::{Info, peers_refresh};


impl<'a> Info<'a> {
    pub fn hosts_are_fresh(&self, now: Time) -> bool {
        self.hosts_timestamp
            .map(|x| x + peers_refresh()*3/2 > now)
            .unwrap_or(false)
    }
}
