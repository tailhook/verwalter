use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender};

use rotor;
use rotor_http::server;
use rotor::mio::tcp::{TcpListener};
use rotor_cantal::{Schedule, connect_localhost, Fsm as CantalFsm};
use rotor_tools::loop_ext::{LoopExt, LoopInstanceExt};

use config::Sandbox;
use shared::SharedState;
use frontend::Public;
use elect::{Election, peers_refresh};
use shared::Id;
use watchdog::{self, Watchdog, Alarm};
use fetch;


rotor_compose!(pub enum Fsm/Seed<Context> {
    Frontend(server::Fsm<Public, TcpListener>),
    Cantal(CantalFsm<Context>),
    Election(Election),
    Watchdog(Watchdog),
    AskLeader(fetch::LeaderFetcher),
});


pub struct Context {
    pub state: SharedState,
    pub cantal: Schedule,
    pub frontend_dir: PathBuf,
    pub log_dir: PathBuf,
    pub sandbox: Sandbox,
}

pub fn main(addr: &SocketAddr, id: Id, hostname: String, name: String,
    state: SharedState, frontend_dir: PathBuf,
    sandbox: &Sandbox, log_dir: PathBuf,
    alarms: Receiver<SyncSender<Alarm>>)
    -> Result<(), io::Error>
{
    let mut creator = rotor::Loop::new(&rotor::Config::new())
        .expect("create loop");
    let schedule = creator.add_and_fetch(Fsm::Cantal, |scope| {
        connect_localhost(scope)
    }).expect("create cantal endpoint");
    state.set_cantal(schedule.clone());

    let fetch_notifier = creator
        .add_and_fetch(Fsm::AskLeader, |s| fetch::create(s))
        .expect("create cantal endpoint");
    state.set_update_notifier(fetch_notifier);

    schedule.set_peers_interval(peers_refresh());
    let mut loop_inst = creator.instantiate(Context {
        state: state.clone(),
        frontend_dir: frontend_dir,
        log_dir: log_dir,
        cantal: schedule.clone(),
        sandbox: sandbox.clone(),
    });
    let listener = TcpListener::bind(&addr).expect("Can't bind address");
    loop_inst.add_machine_with(|scope| {
        server::Fsm::<Public, _>::new(listener, (), scope).wrap(Fsm::Frontend)
    }).expect("Can't add a state machine");
    loop_inst.add_machine_with(|scope| {
        schedule.add_listener(scope.notifier());
        Election::new(id, hostname, name, addr,
                      state, schedule, scope).wrap(Fsm::Election)
    }).expect("Can't add a state machine");

    debug!("Starting alarms");
    while let Ok(snd) = alarms.recv() {
        let alarm = loop_inst.add_and_fetch(
            Fsm::Watchdog, |s| watchdog::create(s))
            .expect("create watchdog");
        snd.send(alarm).expect("send alarm");
    }
    debug!("All alarms received");

    loop_inst.run()
}
