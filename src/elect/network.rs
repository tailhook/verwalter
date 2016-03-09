use std::collections::HashMap;

use time::Timespec;
use rotor::{Machine, EventSet, Scope, Response};
use rotor::void::{unreachable, Void};
use rotor_cantal::Schedule;

use net::Context;
use super::machine;
use super::{Election, Info, Id, PeerInfo};


impl Election {
    pub fn new(id: Id, schedule: Schedule, scope: &mut Scope<Context>)
        -> Response<Election, Void>
    {
        let mach = machine::Machine::new(scope.now());
        let dline = mach.current_deadline();
        Response::ok(Election {
            info: Info::new(id),
            schedule: schedule,
            machine: mach,
        }).deadline(dline)
    }
}

impl Machine for Election {
    type Context = Context;
    type Seed = Void;
    fn create(seed: Self::Seed, _scope: &mut Scope<Context>)
        -> Response<Self, Void>
    {
        unreachable(seed)
    }
    fn ready(self, _events: EventSet, _scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        unimplemented!();
    }
    fn spawned(self, _scope: &mut Scope<Context>) -> Response<Self, Self::Seed>
    {
        unimplemented!();
    }
    fn timeout(self, scope: &mut Scope<Context>) -> Response<Self, Self::Seed>
    {
        let (me, alst) = self.machine.time_passed(&self.info, scope.now());
        match alst.action {
            Some(x) => {
                println!("DO -------> {:?}", x)
            }
            None => {}
        }
        Response::ok(Election { machine: me, ..self})
        .deadline(alst.next_wakeup)
    }
    fn wakeup(mut self, _scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        self.info.all_hosts = self.schedule.get_peers().map(|peers| {
            peers.peers.iter()
            .filter_map(|p| {
                p.id.parse()
                .map_err(|e| error!("Error parsing node id {:?}", p.id)).ok()
                .map(|x| (x, p))
            }).map(|(id, p)| (id, PeerInfo {
                addr: p.primary_addr.as_ref().and_then(|x| x.parse().ok()),
                last_report: p.last_report_direct.map(|x| {
                    Timespec { sec: (x/1000) as i64,
                               nsec: ((x % 1000)*1_000_000) as i32 }
                }),
            })).collect()
        }).unwrap_or_else(HashMap::new);
        // TODO(tailhook) check wakeup time
        println!("Selfinfo {:?}", self.info);
        Response::ok(self)
    }
}
