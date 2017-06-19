use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use futures::{Future, Async};
use futures::sync::{oneshot, mpsc};
use scheduler::{Hash, Schedule};
use tk_easyloop;

use id::Id;
use peer::Peer;
use shared::SharedState;

#[derive(Debug)]
pub struct PrefetchStatus {
    all_schedules: Arc<Mutex<HashMap<Hash, Arc<Schedule>>>>,
    peers: mpsc::UnboundedSender<(Id, Hash)>,
    shutter: oneshot::Sender<()>,
}

pub struct Prefetch {
    shared: SharedState,
    peers: mpsc::UnboundedReceiver<(Id, Hash)>,

    /// Just storage for downloaded schedules. We don't remove anything
    /// from here until actual scheduling is done
    all_schedules: Arc<Mutex<HashMap<Hash, Arc<Schedule>>>>,
}

impl PrefetchStatus {
    // NOTE: execuing under GLOBAL LOCK!!!
    pub fn new(shared: &SharedState, my_schedule: &Option<Arc<Schedule>>)
        -> PrefetchStatus
    {
        let (stx, srx) = oneshot::channel();
        let (ptx, prx) = mpsc::unbounded();
        let shared2 = shared.clone();
        let mut all = HashMap::new();

        my_schedule.as_ref().map(|s| {
            all.insert(s.hash.clone(), s.clone())
        });

        let all = Arc::new(Mutex::new(all));
        let all2 = all.clone();

        shared.mainloop.spawn(move |_handle| {
            Prefetch {
                shared: shared2,
                peers: prx,
                all_schedules: all2,
            }.join(srx.then(|_| Ok(())))
            // TODO(tailhook) join with timeout, maybe?
            .then(|_| {
                debug!("Prefetch loop exit");
                Ok(())
            })
        });

        return PrefetchStatus {
            all_schedules: all,
            peers: ptx,
            shutter: stx,
        };
    }
    pub fn peer_report(&mut self, id: &Id, peer: &Peer) {
        //self.peers.send((id, peer.schedule));
        unimplemented!();
    }
}

impl Future for Prefetch {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        unimplemented!();
    }
}
