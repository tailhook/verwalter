use std::io;
use std::net::SocketAddr;

use rotor;
use rotor::transports::accept;
use rotor_http::HttpServer;
use rotor_http::http1::{Client, Request, Handler, ResponseBuilder};
use mio::EventLoop;
use mio::tcp::TcpListener;
use hyper::uri::RequestUri::{AbsolutePath};

use routing_util::path_component;

struct Context;

#[derive(Clone, Debug)]
pub enum Route {
    Index,
    Static(String),
}

struct Public;

impl Handler<Context> for Public {
    fn request(req: Request, res: &mut ResponseBuilder, ctx: &mut Context) {
        use self::Route::*;
        let path = match req.uri {
            AbsolutePath(ref x) => &x[..],
            // TODO(tailhook) fix AbsoluteUri
            _ => return,  // Do nothing: not found or bad request
        };
        let route = match path_component(&path[..]).0 {
            "" => Index,
            "js" | "css" => Static(path.to_string()),
            _ => return,   // Do nothing: not found or bad request
        };
        debug!("Routed {:?} to {:?}", req, route);
    }
}

pub fn main(addr: &SocketAddr) -> Result<(), io::Error> {
    let mut event_loop = EventLoop::new().unwrap();
    let mut handler = rotor::Handler::new(Context, &mut event_loop);
    handler.add_root(&mut event_loop,
        HttpServer::<_, Public>::new(
            try!(TcpListener::bind(&addr))));
    event_loop.run(&mut handler).unwrap();
    Ok(())
}
