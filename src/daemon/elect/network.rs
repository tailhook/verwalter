use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::exit;
use std::sync::{Mutex, Arc};
use std::time::{SystemTime, Instant, Duration};

use abstract_ns::{self, Resolver};
use futures::{Async, Future};
use tokio_core::net::UdpSocket;
use tokio_core::reactor::Timeout;
use libcantal::{Counter, Integer};
use tk_easyloop::{handle, spawn, timeout_at};

use cell::Cell;
use elect::action::Action;
use elect::{Info};
use elect::{encode};
use elect::machine::{Epoch, Machine};
use elect::settings::MAX_PACKET_SIZE;
use elect::state::ElectionState;
use id::Id;
use peer::Peer;
use shared::{SharedState};
use time_util::ToMsec;


lazy_static! {
    pub static ref BROADCASTS_SENT: Counter = Counter::new();
    pub static ref BROADCASTS_ERRORED: Counter = Counter::new();
    pub static ref PONGS_SENT: Counter = Counter::new();
    pub static ref PONGS_ERRORED: Counter = Counter::new();
    pub static ref LAST_PING_ALL: Integer = Integer::new();
    pub static ref LAST_VOTE: Integer = Integer::new();
    pub static ref LAST_CONFIRM_VOTE: Integer = Integer::new();
    pub static ref LAST_PONG: Integer = Integer::new();
}


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

fn sockets_send(sockets: &[UdpSocket], msg: &[u8], addr: &SocketAddr)
    -> Result<(), ()>
{
    let mut errors = Vec::new();
    for sock in sockets {
        match sock.send_to(msg, addr) {
            Ok(_) => return Ok(()),
            Err(e) => errors.push(e),
        }
    }
    error!("Error sending UDP message: {}",
        errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "));
    return Err(())
}

fn send_all(msg: &[u8], info: &Info, sockets: &[UdpSocket]) {
    for (id, peer) in info.all_hosts {
        if id == info.id {
            // Don't send anything to myself
            continue;
        }
        if let Some(ref addr) = peer.addr {
            debug!("Sending Ping to {} ({})", addr, id);
            sockets_send(sockets, &msg, addr)
                .map(|()| BROADCASTS_SENT.incr(1))
                .map_err(|()| BROADCASTS_ERRORED.incr(1))
                .ok();
        } else {
            debug!("Can't send to {:?}, no address", id);
        }
    }
}

fn execute_action(action: Action, info: &Info, epoch: Epoch,
    sockets: &[UdpSocket], state: &SharedState, last_sent_hash: &mut String)
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
            send_all(&msg, info, sockets);
            LAST_PING_ALL.set(now.to_msec() as i64);
            *last_sent_hash = opt_schedule.as_ref().map(|x| &x.hash[..])
                .unwrap_or("").to_string();
        }
        Vote(id) => {
            info!("[{}] Vote for {}", epoch, id);
            let msg = encode::vote(&info.id, epoch, &id, &state.schedule());
            send_all(&msg, info, sockets);
            LAST_VOTE.set(now.to_msec() as i64);
        }
        ConfirmVote(id) => {
            info!("[{}] Confirm vote {}", epoch, id);
            let msg = encode::vote(&info.id, epoch, &id, &state.schedule());
            let dest = info.all_hosts.get(&id).and_then(|x| x.addr);
            if let Some(ref addr) = dest {
                debug!("Sending (confirm) vote to {} ({:?})", addr, id);
                sockets_send(sockets, &msg, addr).ok();
                LAST_CONFIRM_VOTE.set(now.to_msec() as i64);
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
                sockets_send(sockets, &msg, addr)
                    .map(|()| PONGS_SENT.incr(1))
                    .map_err(|()| PONGS_ERRORED.incr(1))
                    .ok();
                LAST_PONG.set(now.to_msec() as i64);
            } else {
                debug!("Error sending pong to {:?}, no address", id);
            }
        }
    }
}

struct ElectionMachine {
    sockets: Vec<UdpSocket>,
    peers: Cell<Arc<(SystemTime, HashMap<Id, Peer>)>>,
    shared: SharedState,
    machine: Option<Machine>,
    last_schedule_sent: String,
    timer: Timeout,
}

impl Future for ElectionMachine {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Result<Async<()>, ()> {
        let peers = self.peers.get();
        let ref info = Info {
            id: &self.shared.id.clone(),
            hosts_timestamp: Some(peers.0),
            all_hosts: &peers.1,
            debug_force_leader: self.shared.debug_force_leader(),
        };
        let mut me = self.machine.take().expect("machine is always here");
        let dline = me.current_deadline();
        loop {
            // Input messages
            me = self.process_input_messages(me, info);

            // Timeouts
            let (m, act) = me.time_passed(info, Instant::now());
            me = m;
            act.action.map(|x| execute_action(x, &info,
                me.current_epoch(), &self.sockets,
                &self.shared, &mut self.last_schedule_sent));

            let new_dline = me.current_deadline();
            if new_dline == dline {
                break;
            } else {
                self.timer = timeout_at(new_dline);
                match self.timer.poll() {
                    Ok(Async::NotReady) => break,
                    Ok(Async::Ready(())) => continue,
                    Err(_) => unreachable!(),
                }
            }
        }
        self.shared.update_election(ElectionState::from(&me), None);
        self.machine = Some(me);
        Ok(Async::NotReady)
    }
}

impl ElectionMachine {
    fn process_input_messages(&mut self, mut me: Machine, info: &Info)
        -> Machine
    {
        let mut buf = [0u8; MAX_PACKET_SIZE];

        let ref shared = self.shared;
        let ref sockets = self.sockets;
        let ref mut hash = self.last_schedule_sent;
        for socket in &self.sockets {
            while let Ok((n, _)) = socket.recv_from(&mut buf) {
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
                            Instant::now());
                        me = m;
                        act.action.map(|x| execute_action(x, &info,
                            me.current_epoch(), &sockets,
                            shared, hash));
                        shared.update_election(
                            ElectionState::from(&me),
                            msg.schedule.map(|x| (src, x)));
                    }
                    Err(e) => {
                        info!("Error parsing packet {:?}", e);
                    }
                }
            }
        }
        return me;
    }
}

pub fn spawn_election(ns: &abstract_ns::Router, addr: &str,
    state: &SharedState)
    -> Result<(), Box<::std::error::Error>>
{
    let str_addr = addr.to_string();
    let state = state.clone();
    spawn(ns.resolve(addr).map(move |address| {
        let socks = address.at(0).addresses().map(|a| {
                UdpSocket::bind(&a, &handle())
                .map_err(move |e| {
                    error!("Can't bind address {}: {}", a, e);
                    exit(3);
                }).unwrap()
            }).collect();
        spawn(ElectionMachine {
            machine: Some(Machine::new(Instant::now())),
            peers: state.peer_cell(),
            sockets: socks,
            shared: state,
            last_schedule_sent: String::new(),
            timer: timeout_at(Instant::now()),
        });
    }).map_err(move |e| {
        error!("Can't bind address {}: {}", str_addr, e);
        exit(3);
    }));
    Ok(())
}
