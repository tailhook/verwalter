use std::net::SocketAddr;
use std::collections::HashMap;

use time::Timespec;
use rotor::{Machine, EventSet, Scope, Response, PollOpt};
use rotor::mio::udp::UdpSocket;
use rotor::void::{unreachable, Void};
use rotor_cantal::Schedule;

use net::Context;
use super::{machine, encode};
use super::settings::MAX_PACKET_SIZE;
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

fn send_all(msg: &[u8], info: &Info, socket: &UdpSocket) {
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

fn execute_action(action: Action, info: &Info, epoch: Epoch,
    socket: &UdpSocket)
{
    use super::action::Action::*;
    match action {
        PingAll => {
            info!("[{}] Ping all", epoch);
            let msg = encode::ping(&info.id, epoch);
            send_all(&msg, info, socket);
        }
        Vote(id) => {
            info!("[{}] Vote for {}", epoch, id);
            let msg = encode::vote(&info.id, epoch, &id);
            send_all(&msg, info, socket);
        }
        ConfirmVote(id) => {
            info!("[{}] Confirm vote {}", epoch, id);
            let msg = encode::vote(&info.id, epoch, &id);
            if let Some(ref addr) = info.all_hosts.get(&id).and_then(|x| x.addr) {
                debug!("Sending (confirm) vote to {} ({:?})", addr, id);
                socket.send_to(&msg, addr)
                .map_err(|e| info!("Error sending message to {}: {}",
                    addr, e)).ok();
            } else {
                debug!("Error confirming vote to {:?}, no address", id);
            }
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
    fn ready(self, events: EventSet, scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        let mut me = self.machine;
        {
            let ref info = self.info;
            let ref socket = self.socket;
            if events.is_readable() {
                let mut buf = [0u8; MAX_PACKET_SIZE];
                while let Ok(Some((n, _))) = self.socket.recv_from(&mut buf) {
                    // TODO(tailhook) check address?
                    match encode::read_packet(&buf[..n]) {
                        Ok(msg) => {
                            let (m, act) = me.message(&self.info,
                                msg, scope.now());
                            me = m;
                            act.action.map(|x| execute_action(x, &info,
                                me.current_epoch(), &socket));
                        }
                        Err(e) => {
                            info!("Error parsing packet {:?}", e);
                        }
                    }
                }
            }
        }
        let dline = me.current_deadline();
        Response::ok(Election { machine: me, ..self }).deadline(dline)
    }
    fn spawned(self, _scope: &mut Scope<Context>) -> Response<Self, Self::Seed>
    {
        unimplemented!();
    }
    fn timeout(mut self, scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        let (me, wakeup) = {
            let ref info = self.info;
            let ref socket = self.socket;
            let (me, alst) = self.machine.time_passed(
                &self.info, scope.now());
            debug!("Current state {:?} at {:?} -> {:?}",
                me, scope.now(), alst);
            alst.action.map(|x| execute_action(x, &info,
                    me.current_epoch(), &socket));
            (me, alst.next_wakeup)
        };
        Response::ok(Election { machine: me, ..self})
        .deadline(wakeup)
    }
    fn wakeup(mut self, _scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        let new_hosts = self.schedule.get_peers().map(|peers| {
            if self.info.all_hosts.len() != peers.peers.len() {
                info!("Peer number changed {} -> {}",
                    self.info.all_hosts.len(), peers.peers.len());
            }
            self.info.hosts_timestamp = Some(peers.received);
            peers.peers.iter()
            .filter_map(|p| {
                p.id.parse()
                .map_err(|e| error!("Error parsing node id {:?}", p.id)).ok()
                .map(|x| (x, p))
            }).map(|(id, p)| (id, PeerInfo {
                addr: p.primary_addr.as_ref()
                    .and_then(|x| x.parse().ok())
                    // TODO(tailhook) allow to override port
                    .map(|x: SocketAddr| SocketAddr::new(x.ip(), 8379)),
                last_report: p.last_report_direct.map(|x| {
                    Timespec { sec: (x/1000) as i64,
                               nsec: ((x % 1000)*1_000_000) as i32 }
                }),
            })).collect()
        }).unwrap_or_else(HashMap::new);
        self.info.all_hosts = new_hosts;
        let dline = self.machine.current_deadline();
        Response::ok(self).deadline(dline)
    }
}
