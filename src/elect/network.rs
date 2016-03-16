use std::net::SocketAddr;
use std::collections::HashMap;

use time::Timespec;
use rotor::{Machine, EventSet, Scope, Response, PollOpt};
use rotor::mio::udp::UdpSocket;
use rotor::void::{unreachable, Void};
use rotor_cantal::Schedule;

use net::Context;
use shared::SharedState;
use super::{machine, encode};
use super::settings::MAX_PACKET_SIZE;
use super::action::Action;
use super::machine::Epoch;
use super::{Election, Info};
use shared::{Peer, Id};


impl Election {
    pub fn new(id: Id, addr: &SocketAddr,
        state: SharedState, schedule: Schedule, scope: &mut Scope<Context>)
        -> Response<Election, Void>
    {
        let mach = machine::Machine::new(scope.now());
        let dline = mach.current_deadline();
        let sock = match UdpSocket::bound(addr) {
            Ok(x) => x,
            Err(e) => return Response::error(Box::new(e)),
        };
        scope.register(&sock, EventSet::readable() | EventSet::writable(),
            PollOpt::edge()).expect("register socket");
        Response::ok(Election {
            id: id,
            state: state,
            schedule: schedule,
            machine: mach,
            socket: sock,
        }).deadline(dline)
    }
}

fn send_all(msg: &[u8], info: &Info, socket: &UdpSocket) {
    for (id, peer) in info.all_hosts {
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
            info!("[{}] Confirming leadership by pinging everybody", epoch);
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
            let dest = info.all_hosts.get(&id).and_then(|x| x.addr);
            if let Some(ref addr) = dest {
                debug!("Sending (confirm) vote to {} ({:?})", addr, id);
                socket.send_to(&msg, addr)
                .map_err(|e| info!("Error sending message to {}: {}",
                    addr, e)).ok();
            } else {
                debug!("Error confirming vote to {:?}, no address", id);
            }
        }
        Pong(id) => {
            info!("[{}] Pong to a leader {}", epoch, id);
            let msg = encode::pong(&info.id, epoch);
            let dest = info.all_hosts.get(&id).and_then(|x| x.addr);
            if let Some(ref addr) = dest {
                debug!("Sending pong to {} ({:?})", addr, id);
                socket.send_to(&msg, addr)
                .map_err(|e| info!("Error sending message to {}: {}",
                    addr, e)).ok();
            } else {
                debug!("Error sending pong to {:?}, no address", id);
            }
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
            let peers_opt = self.state.peers();
            let empty_map = HashMap::new();
            let peers = peers_opt.as_ref().map(|x| &x.1).unwrap_or(&empty_map);
            let ref info = Info {
                id: &self.id,
                hosts_timestamp: peers_opt.as_ref().map(|x| x.0),
                all_hosts: &peers,
            };
            let ref socket = self.socket;
            if events.is_readable() {
                let mut buf = [0u8; MAX_PACKET_SIZE];
                while let Ok(Some((n, _))) = self.socket.recv_from(&mut buf) {
                    // TODO(tailhook) check address?
                    match encode::read_packet(&buf[..n]) {
                        Ok(msg) => {
                            let (m, act) = me.message(info, msg, scope.now());
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
        unreachable!();
    }
    fn timeout(self, scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        let (me, wakeup) = {
            let peers_opt = self.state.peers();
            let empty_map = HashMap::new();
            let peers = peers_opt.as_ref().map(|x| &x.1).unwrap_or(&empty_map);
            let ref info = Info {
                id: &self.id,
                hosts_timestamp: peers_opt.as_ref().map(|x| x.0),
                all_hosts: &peers,
            };
            let ref socket = self.socket;
            let (me, alst) = self.machine.time_passed(info, scope.now());
            debug!("Current state {:?} at {:?} -> {:?}",
                me, scope.now(), alst);
            alst.action.map(|x| execute_action(x, &info,
                    me.current_epoch(), &socket));
            (me, alst.next_wakeup)
        };
        Response::ok(Election { machine: me, ..self})
        .deadline(wakeup)
    }
    fn wakeup(self, _scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        let oldp = self.state.peers();
        let oldpr = oldp.as_ref();
        let (recv, new_hosts) = if let Some(peers) = self.schedule.get_peers()
        {
            if oldpr.map(|x| x.1.len() != peers.peers.len()).unwrap_or(true) {
                info!("Peer number changed {} -> {}",
                    oldpr.map(|x| x.1.len()).unwrap_or(0), peers.peers.len());
            }
            let map = peers.peers.iter()
                .filter_map(|p| {
                    p.id.parse()
                    .map_err(|e| error!("Error parsing node id {:?}: {}",
                                        p.id, e)).ok()
                    .map(|x| (x, p))
                }).map(|(id, p)| (id, Peer {
                    addr: p.primary_addr.as_ref()
                        .and_then(|x| x.parse().ok())
                        // TODO(tailhook) allow to override port
                        .map(|x: SocketAddr| SocketAddr::new(x.ip(), 8379)),
                    last_report: p.last_report_direct.map(|x| {
                        Timespec { sec: (x/1000) as i64,
                                   nsec: ((x % 1000)*1_000_000) as i32 }
                    }),
                    hostname: p.hostname.clone(),
                })).collect();
            (peers.received, map)
        } else {
            let dline = self.machine.current_deadline();
            return Response::ok(self).deadline(dline);
        };
        self.state.set_peers(recv, new_hosts);
        let dline = self.machine.current_deadline();
        Response::ok(self).deadline(dline)
    }
}
