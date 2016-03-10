use std::net::SocketAddr;
use std::collections::HashMap;

use time::Timespec;
use rotor::{Machine, EventSet, Scope, Response, PollOpt};
use rotor::mio::udp::UdpSocket;
use rotor::void::{unreachable, Void};
use rotor_cantal::Schedule;

use net::Context;
use super::{machine, encode};
use super::action::Action;
use super::machine::Epoch;
use super::{Election, Info, Id, PeerInfo, Capsule, Message};


impl Election {
    pub fn new(id: Id, addr: &SocketAddr,
        schedule: Schedule, scope: &mut Scope<Context>)
        -> Response<Election, Void>
    {
        let mach = machine::Machine::new(scope.now());
        let dline = mach.current_deadline();
        let sock = match UdpSocket::bound(addr) {
            Ok(x) => x,
            Err(e) => return Response::error(Box::new(e)),
        };
        scope.register(&sock, EventSet::readable() | EventSet::writable(),
            PollOpt::edge());
        Response::ok(Election {
            info: Info::new(id),
            schedule: schedule,
            machine: mach,
            socket: sock,
        }).deadline(dline)
    }
}

fn execute_action(action: Action, info: &Info, epoch: Epoch,
    socket: &UdpSocket)
{
    use super::action::Action::*;
    match action {
        PingAll => {
            info!("Ping all, epoch {}", epoch);
            let msg = encode::ping(&info.id, epoch);

            for (id, peer) in &info.all_hosts {
                if let Some(ref addr) = peer.addr {
                    debug!("Sending Ping to {} ({:?})", addr, id);
                    socket.send_to(&msg, addr)
                    .map_err(|e| info!("Error sending message to {}: {}",
                        addr, e)).ok();
                } else {
                    debug!("Can't send to {:?}, no address", id);
                }
            }
        }
        Vote(_) => {
            unimplemented!();
        }
        ConfirmVote(_) => {
            unimplemented!();
        }
        Pong => {
            unimplemented!();
        }
        PingNew => {
            unimplemented!();
        }
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
    fn ready(self, events: EventSet, _scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        if events.is_readable() {
            unimplemented!();
        } else {
            let dline = self.machine.current_deadline();
            Response::ok(self).deadline(dline)
        }
    }
    fn spawned(self, _scope: &mut Scope<Context>) -> Response<Self, Self::Seed>
    {
        unimplemented!();
    }
    fn timeout(mut self, scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        let (me, alst) = self.machine.time_passed(&self.info, scope.now());
        debug!("Current state {:?} at {:?} -> {:?}", me, scope.now(), alst);
        match alst.action {
            Some(x) => {
                execute_action(x, &self.info,
                    me.current_epoch(), &self.socket);
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
