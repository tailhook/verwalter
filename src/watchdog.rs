use std::cmp::max;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use std::process::exit;

use time;
use time::{Timespec, get_time};
use rotor::{Machine, EventSet, Scope, Response, Notifier, GenericScope};
use rotor::void::{unreachable, Void};

use net::Context;

pub struct Watchdog(Arc<Mutex<Option<Timespec>>>);

pub struct Alarm(Arc<Mutex<Option<Timespec>>>, Notifier);

pub struct AlarmGuard<'a>(&'a mut Alarm);

/// This is a guard with exits with specified code on Drop
pub struct ExitOnReturn(pub i32);

impl Watchdog {
    fn proceed(self, scope: &mut Scope<Context>) -> Response<Self, Void> {
        let dline = *self.0.lock().expect("watchdog lock");
        if let Some(deadline) = dline {
            Response::ok(self)
                .deadline(scope.now() + Duration::from_millis(
                    max((deadline - get_time()).num_milliseconds(), 0) as u64))
        } else {
            Response::ok(self)
        }
    }
}

impl Machine for Watchdog {
    type Context = Context;
    type Seed = Void;
    fn create(seed: Self::Seed, _scope: &mut Scope<Context>)
        -> Response<Self, Void>
    {
        unreachable(seed)
    }
    fn ready(self, _events: EventSet, scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        self.proceed(scope)  // spurious events
    }
    fn spawned(self, _scope: &mut Scope<Context>) -> Response<Self, Self::Seed>
    {
        unreachable!();
    }
    fn timeout(self, scope: &mut Scope<Context>) -> Response<Self, Self::Seed>
    {
        let time_opt = *self.0.lock().expect("lock alarm");
        match time_opt {
            Some(time) if time >= get_time() => {
                error!("Alarm failed. Exiting with exit code 91");
                exit(91);
            }
            _ => self.proceed(scope),
        }
    }
    fn wakeup(self, scope: &mut Scope<Context>) -> Response<Self, Self::Seed>
    {
        self.proceed(scope)  // this re-arms the timer
    }
}

impl Alarm {
    pub fn after(&mut self, delay: Duration) -> AlarmGuard {
        *self.0.lock().expect("lock alarm") = Some(get_time() +
            time::Duration::seconds(delay.as_secs() as i64)
            + time::Duration::nanoseconds(delay.subsec_nanos() as i64));
        self.1.wakeup().expect("wakeup watchdog");
        AlarmGuard(self)
    }
}

impl<'a> Drop for AlarmGuard<'a> {
    fn drop(&mut self) {
        (self.0).0.lock().expect("lock alarm").take();
    }
}

impl Drop for ExitOnReturn {
    fn drop(&mut self) {
        exit(self.0);
    }
}

pub fn create<S: GenericScope>(scope: &mut S)
    -> Response<(Watchdog, Alarm), Void>
{
    let arc = Arc::new(Mutex::new(None));
    Response::ok((
        Watchdog(arc.clone()),
        Alarm(arc, scope.notifier()),
    ))
}

