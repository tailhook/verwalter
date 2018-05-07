use std::sync::Arc;

use async_slot as slot;
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::Stream;

use apply::ApplyData;
use scheduler::Schedule;
use watchdog;


#[derive(Debug)]
pub enum Request {
    // this one isn't actually send over channel, but we're using it for
    // simplifying interface
    NewSchedule(Arc<Schedule>),
    ForceRerender,
}

#[derive(Debug, Clone)]
pub struct Responder(Arc<Internal>);

#[derive(Debug)]
struct Internal {
    schedule_tx: slot::Sender<Arc<Schedule>>,
    tx: UnboundedSender<Request>,
}

pub struct ResponderInit {
    rx: UnboundedReceiver<Request>,
    // coalesce subsequent schedules, prioritize over requests
    schedule_rx: slot::Receiver<Arc<Schedule>>,
    apply_tx: slot::Sender<ApplyData>,
}

impl Responder {
    pub fn new(apply_tx: slot::Sender<ApplyData>) -> (Responder, ResponderInit) {
        let (tx, rx) = unbounded();
        let (schedule_tx, schedule_rx) = slot::channel();
        let resp = Responder(Arc::new(Internal {
            tx,
            schedule_tx,
        }));
        let init = ResponderInit {
            rx,
            apply_tx,
            schedule_rx,
        };
        return (resp, init);
    }
    pub fn new_schedule(&self, sched: Arc<Schedule>) {
        self.0.schedule_tx.swap(sched)
            .expect("schedule channel works");
    }
    pub fn force_rerender(&self) {
        self.0.tx.unbounded_send(Request::ForceRerender)
            .expect("responder channel works");
    }
}

pub fn run(init: ResponderInit) {
    let _guard = watchdog::ExitOnReturn(83);
    let stream = init.schedule_rx.map(Request::NewSchedule)
        .select(init.rx);
    for request in stream.wait() {
        let request = request.expect("stream not closed");
        debug!("Incoming request {:?}", request);
        match request {
            Request::NewSchedule(schedule) => {
                unimplemented!();
            }
            Request::ForceRerender => {
                unimplemented!();
            }
        }
    }
}
