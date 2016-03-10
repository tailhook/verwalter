use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use rotor;
use rotor_http::server;
use rotor::mio::tcp::{TcpListener};
use rotor_cantal::{Schedule, connect_localhost, Fsm as CantalFsm};
use rotor_tools::loop_ext::LoopExt;

use config::Config;
use frontend::Public;
use elect::{Election, Id};


rotor_compose!(pub enum Fsm/Seed<Context> {
    Frontend(server::Fsm<Public, TcpListener>),
    Cantal(CantalFsm<Context>),
    Election(Election),
});


pub struct Context {
    pub config: Arc<RwLock<Config>>,
    pub schedule: Schedule,
}


pub fn main(addr: &SocketAddr, id: Id, cfg: Arc<RwLock<Config>>)
    -> Result<(), io::Error>
{
    let mut creator = rotor::Loop::new(&rotor::Config::new())
        .expect("create loop");
    let schedule = creator.add_and_fetch(Fsm::Cantal, |scope| {
        connect_localhost(scope)
    }).expect("create cantal endpoint");
    schedule.set_peers_interval(Duration::new(10, 0));
    let mut loop_inst = creator.instantiate(Context {
        config: cfg,
        schedule: schedule.clone(),
    });
    let listener = TcpListener::bind(&addr).expect("Can't bind address");
    loop_inst.add_machine_with(|scope| {
        server::Fsm::<Public, _>::new(listener, scope).wrap(Fsm::Frontend)
    }).expect("Can't add a state machine");
    loop_inst.add_machine_with(|scope| {
        Election::new(id, addr, schedule, scope).wrap(Fsm::Election)
    }).expect("Can't add a state machine");
    loop_inst.run()
}
