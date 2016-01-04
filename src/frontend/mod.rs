use std::io;
use std::io::Read;
use std::path::Path;
use std::path::Component::ParentDir;
use std::fs::File;

use time::Duration;
use hyper::header::{ContentLength, ContentType};
use rotor::Scope;
use rotor_stream::Deadline;
use rotor_http::server::{Server, Response, RecvMode, Head};
use rotor_http::server::{Context as HttpContext};
use hyper::uri::RequestUri::{AbsolutePath};
use hyper::status::StatusCode;
use hyper::mime::{Mime, TopLevel, SubLevel};
use rustc_serialize::json::{ToJson, as_pretty_json};

use routing_util::path_component;
use config::Config;
use net::Context;

#[derive(Clone, Debug)]
pub enum ApiRoute {
    Config,
}

#[derive(Clone, Debug, Copy)]
pub enum Format {
    Json,
    Plain,
}

#[derive(Clone, Debug)]
pub enum Route {
    Index,
    Static(String),
    Api(ApiRoute, Format),
}

pub struct Public(Route);

fn read_file<P:AsRef<Path>>(path: P, res: &mut Response)
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
    res.status(StatusCode::Ok);
    res.add_header(ContentLength(buf.len() as u64)).unwrap();
    // TODO(tailhook) guess mime type
    res.done_headers().unwrap();
    res.write_body(&buf);
    res.done();
    Ok(())
}

fn parse_api(path: &str) -> Option<Route> {
    use self::Route::*;
    use self::ApiRoute::*;
    use self::Format::*;
    match path_component(&path[..]) {
        ("config", "") => Some(Api(Config, Json)),
        ("config.pretty", "") => Some(Api(Config, Plain)),
        _ => None,
    }
}

fn serve_api(context: &Context, route: &ApiRoute, format: Format,
    res: &mut Response)
    -> Result<(), io::Error>
{
    use self::ApiRoute::*;
    use self::Format::*;
    let data = match *route {
        Config => context.config.read().unwrap().to_json(),
    };

    res.status(StatusCode::Ok);
    res.add_header(ContentType(
            Mime(TopLevel::Application, SubLevel::Json, vec![]))).unwrap();
    let data = match format {
        Json => format!("{}", data),
        Plain => format!("{}", as_pretty_json(&data)),
    };
    res.add_header(ContentLength(data.as_bytes().len() as u64)).unwrap();
    res.done_headers().unwrap();
    res.write_body(data.as_bytes());
    res.done();
    Ok(())
}

impl HttpContext for Context {
    // Defaults for now
}

impl Server<Context> for Public {
    fn headers_received(head: &Head, _scope: &mut Scope<Context>)
        -> Result<(Self, RecvMode, Deadline), StatusCode>
    {
        use self::Route::*;
        let uri = match head.uri {
            AbsolutePath(ref x) => &x[..],
            // TODO(tailhook) fix AbsoluteUri
            _ => return Err(StatusCode::NotFound),
        };
        let path = match uri.find('?') {
            Some(x) => &uri[..x],
            None => uri,
        };
        let route = match path_component(&path[..]) {
            ("", _) => Some(Index),
            ("js", _) | ("css", _) => Some(Static(path.to_string())),
            ("v1", suffix) => parse_api(suffix),
            (_, _) => None,
        };
        debug!("Routed {:?} to {:?}", head, route);
        route
        .map(|route| (Public(route), RecvMode::Buffered(1024),
            Deadline::now() + Duration::seconds(120)))
        .ok_or(StatusCode::NotFound)
    }
    fn request_start(self, _head: Head, _res: &mut Response,
        _scope: &mut Scope<Context>)
        -> Option<Self>
    {
        Some(self)
    }
    fn request_received(self, _data: &[u8], res: &mut Response,
        scope: &mut Scope<Context>)
        -> Option<Self>
    {
        use self::Route::*;
        let iores = match *&self.0 {
            Index => read_file("public/index.html", res),
            Static(ref x) => read_file(format!("public/{}", &x[1..]), res),
            Api(ref route, fmt) => serve_api(scope, route, fmt, res),
        };
        match iores {
            Ok(()) => {}
            Err(ref e) if e.kind() == io::ErrorKind::NotFound => {
                scope.emit_error_page(StatusCode::NotFound, res);
            }
            Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {
                scope.emit_error_page(StatusCode::Forbidden, res);
            }
            Err(e) => {
                info!("Error serving {:?}: {}", self.0, e);
                scope.emit_error_page(StatusCode::InternalServerError, res);
            }
        }
        None
    }
    fn request_chunk(self, _chunk: &[u8], _response: &mut Response,
        _scope: &mut Scope<Context>)
        -> Option<Self>
    {
        unreachable!();
    }

    /// End of request body, only for Progressive requests
    fn request_end(self, _response: &mut Response, _scope: &mut Scope<Context>)
        -> Option<Self>
    {
        unreachable!();
    }

    fn timeout(self, _response: &mut Response, _scope: &mut Scope<Context>)
        -> Option<(Self, Deadline)>
    {
        unimplemented!();
    }
    fn wakeup(self, _response: &mut Response, _scope: &mut Scope<Context>)
        -> Option<Self>
    {
        unimplemented!();
    }
}
