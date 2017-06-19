use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use futures::{Future, Async};
use futures::sync::oneshot;
use scheduler::{Hash, Schedule};
use tk_easyloop;

use shared::SharedState;

#[derive(Debug)]
pub struct PrefetchStatus {
    all_schedules: Arc<Mutex<HashMap<Hash, Arc<Schedule>>>>,
    shutter: oneshot::Sender<()>,
}

pub struct Prefetch {
    shared: SharedState,

    /// Just storage for downloaded schedules. We don't remove anything
    /// from here until actual scheduling is done
    all_schedules: Arc<Mutex<HashMap<Hash, Arc<Schedule>>>>,
}

impl PrefetchStatus {
    // NOTE: execuing under GLOBAL LOCK!!!
    pub fn new(shared: &SharedState) -> PrefetchStatus {
        let (tx, rx) = oneshot::channel();
        let shared2 = shared.clone();
        let all = Arc::new(Mutex::new(HashMap::new()));
        let all2 = all.clone();

        shared.mainloop.spawn(move |_handle| {
            Prefetch {
                shared: shared2,
                all_schedules: all2,
            }.join(rx.then(|_| Ok(())))
            // TODO(tailhook) join with timeout, maybe?
            .then(|_| {
                debug!("Prefetch loop exit");
                Ok(())
            })
        });

        return PrefetchStatus {
            all_schedules: all,
            shutter: tx,
        };
    }
}

impl Future for Prefetch {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!();
    }
}
