use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use rotor;
use rotor_http::HttpServer;
use mio::tcp::TcpListener;
use mio::EventLoop;

use config::Config;
use frontend::Public;


pub struct Context {
    pub config: Arc<RwLock<Config>>,
}


pub fn main(addr: &SocketAddr, cfg: Arc<RwLock<Config>>)
    -> Result<(), io::Error>
{
    let mut event_loop = EventLoop::new().unwrap();
    let mut handler = rotor::Handler::new(Context {
        config: cfg,
    }, &mut event_loop);
    handler.add_root(&mut event_loop,
        HttpServer::<_, Public>::new(
            try!(TcpListener::bind(&addr))));
    event_loop.run(&mut handler).unwrap();
    Ok(())
}
