use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;

use rotor;
use rotor_http::server;
use rotor::mio::tcp::{TcpListener};
use rotor_cantal::{Schedule, connect_localhost, Fsm as CantalFsm};
use rotor_tools::loop_ext::LoopExt;

use shared::SharedState;
use frontend::Public;
use elect::{Election, peers_refresh};
use shared::Id;


rotor_compose!(pub enum Fsm/Seed<Context> {
    Frontend(server::Fsm<Public, TcpListener>),
    Cantal(CantalFsm<Context>),
    Election(Election),
});


pub struct Context {
    pub state: SharedState,
    pub schedule: Schedule,
    pub frontend_dir: PathBuf,
}

pub fn main(addr: &SocketAddr, id: Id, hostname: String,
    state: SharedState, frontend_dir: PathBuf)
    -> Result<(), io::Error>
{
    let mut creator = rotor::Loop::new(&rotor::Config::new())
        .expect("create loop");
    let schedule = creator.add_and_fetch(Fsm::Cantal, |scope| {
        connect_localhost(scope)
    }).expect("create cantal endpoint");
    schedule.set_peers_interval(peers_refresh());
    let mut loop_inst = creator.instantiate(Context {
        state: state.clone(),
        frontend_dir: frontend_dir,
        schedule: schedule.clone(),
    });
    let listener = TcpListener::bind(&addr).expect("Can't bind address");
    loop_inst.add_machine_with(|scope| {
        server::Fsm::<Public, _>::new(listener, scope).wrap(Fsm::Frontend)
    }).expect("Can't add a state machine");
    loop_inst.add_machine_with(|scope| {
        schedule.add_listener(scope.notifier());
        Election::new(id, hostname, addr,
                      state, schedule, scope).wrap(Fsm::Election)
    }).expect("Can't add a state machine");
    loop_inst.run()
}
