use std::sync::{Arc, RwLock, Mutex};
use futures::{self, Stream};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};


#[derive(Debug)]
pub struct Sender<T: Clone>(Arc<Internal<T>>);

#[derive(Debug)]
pub struct Cell<T: Clone>(Arc<Internal<T>>, UnboundedReceiver<()>);

#[derive(Debug)]
struct Internal<T: Clone> {
    current_value: RwLock<T>,
    notifiers: Mutex<Vec<UnboundedSender<()>>>,
}

impl<T: Clone> Sender<T> {
    pub fn new(initial_value: T) -> Sender<T> {
        Sender(Arc::new(Internal {
            current_value: RwLock::new(initial_value),
            notifiers: Mutex::new(Vec::new()),
        }))
    }
    pub fn cell(&self) -> Cell<T> {
        let (tx, rx) = unbounded();
        self.0.notifiers.lock().expect("cell is not poisoned")
            .push(tx);
        Cell(self.0.clone(), rx)
    }
    pub fn set(&self, value: T) {
        *self.0.current_value.write().expect("cell is not poisoned")
            = value;
        self.0.notifiers.lock().expect("cell is not poisoned").retain(|s| {
            s.unbounded_send(()).is_ok()
        })
    }
    pub fn get(&self) -> T {
        self.0.current_value
            .read().expect("cell is not poisoned")
            .clone()
    }
}

impl<T: Clone> Cell<T> {
    /// Get's data from cell and also subscribes current task for updates
    pub fn get(&mut self) -> T {
        while let futures::Async::Ready(x) = self.1.poll()
            .expect("unbounded receiver can't fail")
        {
            if x.is_none() { panic!("sender is removed in cell"); }
        }
        self.0.current_value
            .read().expect("cell is not poisoned")
            .clone()
    }
}
