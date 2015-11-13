use std::io;
use std::io::Read;
use std::path::Path;
use std::path::Component::ParentDir;
use std::fs::File;
use std::net::SocketAddr;

use rotor;
use rotor::transports::accept;
use rotor_http::HttpServer;
use rotor_http::http1::{Client, Request, Handler, ResponseBuilder};
use mio::EventLoop;
use mio::tcp::TcpListener;
use hyper::uri::RequestUri::{AbsolutePath};
use hyper::status::StatusCode;

use routing_util::path_component;

struct Context;

#[derive(Clone, Debug)]
pub enum Route {
    Index,
    Static(String),
}

struct Public;

fn read_file<P:AsRef<Path>>(path: P, res: &mut ResponseBuilder)
    -> Result<(), io::Error>
{
    let path = path.as_ref();
    for cmp in path.components() {
        if matches!(cmp, ParentDir) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                "Parent dir `..` path components are not allowed"));
        }
    }
    let mut file = try!(File::open(path));
    let mut buf = Vec::with_capacity(1024);
    try!(file.read_to_end(&mut buf));
    res.set_status(StatusCode::Ok);
    res.put_body(buf);
    Ok(())
}

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
        let iores = match route {
            Index => read_file("public/index.html", res),
            Static(ref x) => read_file(format!("public/{}", &x[1..]), res),
        };
        match iores {
            Ok(()) => {}
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                return; // 404 by default
            }
            Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {
                res.set_status(StatusCode::Forbidden);
                return;
            }
            Err(e) => {
                info!("Error serving {:?}: {}", route, e);
                res.set_status(StatusCode::Forbidden);
                return;
            }
        }
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
