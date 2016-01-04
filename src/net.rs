use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use rotor;
use rotor_stream::{Accept, Stream};
use rotor_http::server::{Parser};
use mio::tcp::{TcpListener, TcpStream};
use mio::EventLoop;

use config::Config;
use frontend::Public;


pub struct Context {
    pub config: Arc<RwLock<Config>>,
}


pub fn main(addr: &SocketAddr, cfg: Arc<RwLock<Config>>)
    -> Result<(), io::Error>
{
    let mut event_loop = EventLoop::new().expect("Can't create event loop");
    let mut handler = rotor::Handler::new(Context {
        config: cfg,
    }, &mut event_loop);
    let listener = TcpListener::bind(&addr).expect("Can't bind address");
    let ok = handler.add_machine_with(&mut event_loop, |scope| {
        Accept::<TcpListener, TcpStream,
            Stream<Context, _, Parser<Public>>>::new(listener, scope)
    }).is_ok();
    assert!(ok); // TODO
    event_loop.run(&mut handler).expect("Error running event loop");
    Ok(())
}
