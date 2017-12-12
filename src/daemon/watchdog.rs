use std::time::{Instant, Duration};
use std::process::exit;

use futures::{IntoFuture, Future};
use futures::future::Either;
use futures::sync::oneshot::{channel, Sender};
use tokio_core::reactor::{Remote, Handle, Timeout};
use tk_easyloop::handle;


lazy_static! {
    static ref WATCHDOG_HANDLE: Remote = handle().remote().clone();
}


/// This is a guard with exits with specified code on Drop
pub struct ExitOnReturn(pub i32);

pub struct Alarm {
    // we rely that dropping Sender immediately sends Error on a channel
    _stop: Sender<()>,
}


impl Drop for ExitOnReturn {
    fn drop(&mut self) {
        exit(self.0);
    }
}

pub fn init() {
    WATCHDOG_HANDLE.clone(); // init handle
}

fn spawn<F, R>(f: F)
    where F: FnOnce(&Handle) -> R + Send + 'static,
            R: IntoFuture<Item = (), Error = ()>,
            R::Future: 'static
{
    WATCHDOG_HANDLE.spawn(f)
}

impl Alarm {
    pub fn new(delay: Duration, name: &'static str) -> Alarm {
        let (tx, rx) = channel();
        let deadline = Instant::now() + delay;
        spawn(move |handle| {
            Timeout::new_at(deadline, handle)
            .expect("can always add a timeout")
            .map_err(|_| { unreachable!(); })
            .select2(rx)
            .then(move |res| {
                match res {
                    Ok(Either::A(((), _))) => {
                        error!("Alarm {:?} failed. Exiting with exit code 91",
                            name);
                        exit(91);
                    }
                    Err(Either::B((_, _))) => {
                        debug!("Alarm {:?} canceled. That's fine", name);
                        Ok(())
                    }
                    _ => unreachable!(),
                }
            })
        });
        Alarm {
            _stop: tx,
        }
    }
}
