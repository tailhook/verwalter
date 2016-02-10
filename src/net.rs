use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use rotor;
use rotor_http::ServerFsm;
use rotor::mio::tcp::TcpListener;

use config::Config;
use frontend::Public;


pub struct Context {
    pub config: Arc<RwLock<Config>>,
}


pub fn main(addr: &SocketAddr, cfg: Arc<RwLock<Config>>)
    -> Result<(), io::Error>
{
    let loop_creator = rotor::Loop::new(&rotor::Config::new())
        .expect("Can't create loop");
    let mut loop_inst = loop_creator.instantiate(Context {
        config: cfg,
    });
    let listener = TcpListener::bind(&addr).expect("Can't bind address");
    loop_inst.add_machine_with(|scope| {
        ServerFsm::<Public, _>::new(listener, scope)
    }).expect("Can't add a state machine");
    loop_inst.run()
}
