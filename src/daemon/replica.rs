use futures::{Future, Async};
use tk_easyloop;

use shared::SharedState;

pub struct Replica {
    shared: SharedState,
}

impl Replica {
    pub fn new(&self, shared: &SharedState) -> Replica {
        Replica {
            shared: shared.clone(),
        }
    }
}

impl Future for Replica {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!();
    }
}
