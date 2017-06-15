use futures::{Future, Async};
use tk_easyloop;

use shared::SharedState;

pub struct Prefetch {
    shared: SharedState,
}

impl Prefetch {
    pub fn new(shared: &SharedState) -> Prefetch {
        Prefetch {
            shared: shared.clone(),
        }
    }
}

impl Future for Prefetch {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!();
    }
}
