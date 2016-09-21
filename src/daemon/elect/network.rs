use std::net::SocketAddr;
use std::time::{SystemTime, Duration};
use std::sync::Mutex;
use std::collections::HashMap;

use time::{Timespec, get_time};
use rotor::{Machine, EventSet, Scope, Response, PollOpt};
use rotor::mio::udp::UdpSocket;
use rotor::void::{unreachable, Void};
use rotor_cantal::{Schedule as Cantal};

use net::Context;
use shared::{SharedState};
use super::{machine, encode};
use super::settings::MAX_PACKET_SIZE;
use super::action::Action;
use super::machine::Epoch;
use super::state::ElectionState;
use super::{Election, Info};
use shared::{Peer, Id};

/// This structure allows to make log less chatty
///
/// We write on INFO log-level once and hour, and use debug loglevel otherwise
enum LogTracker {
    Nothing,
    Pong(SystemTime, Id),
    PingAll(SystemTime),
}

lazy_static! {
    static ref LOG_TRACKER: Mutex<LogTracker> = Mutex::new(LogTracker::Nothing);
}

impl Election {
    pub fn new(id: Id, hostname: String, name: String, addr: &SocketAddr,
        debug_force_leader: bool,
        state: SharedState, cantal: Cantal, scope: &mut Scope<Context>)
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
            addr: addr.clone(),
            hostname: hostname,
            name: name,
            state: state,
            last_schedule_sent: String::new(),
            cantal: cantal,
            machine: mach,
            socket: sock,
            debug_force_leader: debug_force_leader,
        }).deadline(dline)
    }
}

fn send_all(msg: &[u8], info: &Info, socket: &UdpSocket) {
    for (id, peer) in info.all_hosts {
        if id == info.id {
            // Don't send anything to myself
            continue;
        }
        if let Some(ref addr) = peer.addr {
            debug!("Sending Ping to {} ({})", addr, id);
            socket.send_to(&msg, addr)
            .map_err(|e| info!("Error sending message to {}: {}",
                addr, e)).ok();
        } else {
            debug!("Can't send to {:?}, no address", id);
        }
    }
}

fn execute_action(action: Action, info: &Info, epoch: Epoch,
    socket: &UdpSocket, state: &SharedState, last_sent_hash: &mut String)
{
    use super::action::Action::*;
    let now = SystemTime::now();
    let log_time = now - Duration::new(3600, 0);
    match action {
        PingAll => {
            let opt_schedule = state.owned_schedule();
            match *LOG_TRACKER.lock().unwrap() {
                LogTracker::PingAll(ref mut tm) if *tm > log_time => {
                    debug!(
                        "[{}] Confirming leadership by sending shedule {:?}",
                        epoch, opt_schedule.as_ref().map(|x| &x.hash));
                }
                ref mut node => {
                    *node = LogTracker::PingAll(now);
                    info!(
                        "[{}] Confirming leadership by sending shedule {:?}",
                        epoch, opt_schedule.as_ref().map(|x| &x.hash));
                }
            };
            let msg = encode::ping(&info.id, epoch, &opt_schedule);
            send_all(&msg, info, socket);
            *last_sent_hash = opt_schedule.as_ref().map(|x| &x.hash[..])
                .unwrap_or("").to_string();
        }
        Vote(id) => {
            info!("[{}] Vote for {}", epoch, id);
            let msg = encode::vote(&info.id, epoch, &id, &state.schedule());
            send_all(&msg, info, socket);
        }
        ConfirmVote(id) => {
            info!("[{}] Confirm vote {}", epoch, id);
            let msg = encode::vote(&info.id, epoch, &id, &state.schedule());
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
            match *LOG_TRACKER.lock().unwrap() {
                LogTracker::Pong(ref mut tm, ref tr_id)
                if *tm > log_time && *tr_id == id
                => {
                    debug!("[{}] Pong to a leader {}", epoch, id);
                }
                ref mut node => {
                    *node = LogTracker::Pong(now, id.clone());
                    info!("[{}] Pong to a leader {}", epoch, id);
                }
            }
            let msg = encode::pong(&info.id, epoch, &state.schedule());
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
    fn ready(mut self, events: EventSet, scope: &mut Scope<Context>)
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
                debug_force_leader: self.debug_force_leader,
            };
            let ref socket = self.socket;
            let ref state = self.state;
            let ref mut hash = self.last_schedule_sent;
            if events.is_readable() {
                let mut buf = [0u8; MAX_PACKET_SIZE];
                while let Ok(Some((n, _))) = self.socket.recv_from(&mut buf) {
                    // TODO(tailhook) check address?
                    match encode::read_packet(&buf[..n]) {
                        Ok(msg) => {
                            if &msg.source == info.id {
                                info!("Message from myself {:?}", msg);
                                continue;
                            }
                            let src = msg.source;
                            let (m, act) = me.message(info,
                                (src.clone(), msg.epoch, msg.message),
                                scope.now());
                            me = m;
                            act.action.map(|x| execute_action(x, &info,
                                me.current_epoch(), &socket, state, hash));
                            state.update_election(
                                ElectionState::from(&me, scope),
                                msg.schedule.map(|x| (src, x)));
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
    fn timeout(mut self, scope: &mut Scope<Context>)
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
                debug_force_leader: self.debug_force_leader,
            };
            let ref socket = self.socket;
            let ref state = self.state;
            let ref mut hash = self.last_schedule_sent;
            let (me, alst) = self.machine.time_passed(info, scope.now());
            debug!("Current state {:?} at {:?} -> {:?}",
                me, scope.now(), alst);
            alst.action.map(|x| execute_action(x, &info,
                    me.current_epoch(), &socket, state, hash));
            (me, alst.next_wakeup)
        };
        let elect = ElectionState::from(&me, scope);
        scope.state.update_election(elect, None);
        Response::ok(Election { machine: me, ..self})
        .deadline(wakeup)
    }
    fn wakeup(self, _scope: &mut Scope<Context>)
        -> Response<Self, Self::Seed>
    {
        let oldp = self.state.peers();
        let oldpr = oldp.as_ref();
        // TODO(tailhook) optimize peer copy when unneeded
        let (recv, new_hosts) = if let Some(peers) = self.cantal.get_peers()
        {
            let mut map = peers.peers.iter()
                .filter_map(|p| {
                    p.id.parse()
                    .map_err(|e| error!("Error parsing node id {:?}: {}",
                                        p.id, e)).ok()
                    .map(|x| (x, p))
                })
                // Cantal has a bug (or a feature) of adding itself to peers
                .filter(|&(ref id, _)| id != &self.id)
                .map(|(id, p)| (id, Peer {
                    addr: p.primary_addr.as_ref()
                        .and_then(|x| x.parse().ok())
                        // TODO(tailhook) allow to override port
                        .map(|x: SocketAddr| SocketAddr::new(x.ip(), 8379)),
                    last_report: p.last_report_direct.map(|x| {
                        Timespec { sec: (x/1000) as i64,
                                   nsec: ((x % 1000)*1_000_000) as i32 }
                    }),
                    hostname: p.hostname.clone(),
                    name: p.name.clone(),
                })).collect::<HashMap<_, _>>();
            // We skip host there and add it here, to make sure
            // we have correct host info in the list
            map.insert(self.id.clone(), Peer {
                addr: Some(self.addr),
                last_report: Some(get_time()),
                hostname: self.hostname.clone(),
                name: self.name.clone(),
            }).map(|_| unreachable!());
            if oldpr.map(|x| x.1.len() != map.len()).unwrap_or(true) {
                info!("Peer number changed {} -> {}",
                    oldpr.map(|x| x.1.len()).unwrap_or(0), map.len());
            }
            (peers.received, map)
        } else {
            // Note we just return here, because the (owned) schedule can't be
            // changed while there is no peers yet
            let dline = self.machine.current_deadline();
            return Response::ok(self).deadline(dline);
        };
        self.state.set_peers(recv, new_hosts);

        let dline = self.machine.current_deadline();
        Response::ok(self).deadline(dline)
    }
}
