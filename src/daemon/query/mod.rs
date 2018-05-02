use std::sync::Arc;

use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use futures::Stream;

#[derive(Debug)]
struct Request {
}

#[derive(Debug, Clone)]
pub struct Responder(Arc<Internal>);

#[derive(Debug)]
struct Internal {
    tx: UnboundedSender<Request>,
}

pub struct ResponderInit {
    rx: UnboundedReceiver<Request>,
}

impl Responder {
    pub fn new() -> (Responder, ResponderInit) {
        let (tx, rx) = unbounded();
        let resp = Responder(Arc::new(Internal {
            tx,
        }));
        let init = ResponderInit {
            rx,
        };
        return (resp, init);
    }
}

pub fn run(init: ResponderInit) {
    for request in init.rx.wait() {
        debug!("Incoming request {:?}", request);
    }
}
