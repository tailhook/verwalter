use std::io;
use std::io::Read;
use std::path::Path;
use std::path::Component::ParentDir;
use std::fs::File;
use std::time::{Duration};

use rotor::{Scope, Time};
use rotor_http::server::{Server, Response, RecvMode, Head};
use rotor_http::server::{Context as HttpContext};
use rustc_serialize::Encodable;
use rustc_serialize::json::{ToJson, as_json, as_pretty_json};

use net::Context;

#[derive(Clone, Debug)]
pub enum ApiRoute {
    Config,
    Peers,
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

fn path_component(path: &str) -> (&str, &str) {
    let path = if path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    match path.bytes().position(|x| x == b'/') {
        Some(end) => (&path[..end], &path[end+1..]),
        None => {
            let end = path.bytes().position(|x| x == b'.')
                .unwrap_or(path.as_bytes().len());
            (&path[..end], "")
        }
    }
}

fn suffix(path: &str) -> &str {
    match path.bytes().rposition(|x| x == b'.' || x == b'/') {
        Some(i) if path.as_bytes()[i] == b'.' => &path[i+1..],
        Some(_) => "",
        None => "",
    }
}

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
    res.status(200, "OK");
    res.add_length(buf.len() as u64).unwrap();
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
    match path_component(path) {
        ("config", "") => Some(Api(Config,
            if suffix(path) == "pretty" { Plain } else { Json })),
        ("peers", "") => Some(Api(Peers,
            if suffix(path) == "pretty" { Plain } else { Json })),
        _ => None,
    }
}

fn respond<T: Encodable>(res: &mut Response, format: Format, data: T)
    -> Result<(), io::Error>
{
    res.status(200, "OK");
    res.add_header("Content-Type", b"application/json").unwrap();
    let data = match format {
        Format::Json => format!("{}", as_json(&data)),
        Format::Plain => format!("{}", as_pretty_json(&data)),
    };
    res.add_length(data.as_bytes().len() as u64).unwrap();
    res.done_headers().unwrap();
    res.write_body(data.as_bytes());
    res.done();
    Ok(())
}

fn serve_api(context: &Context, route: &ApiRoute, format: Format,
    res: &mut Response)
    -> Result<(), io::Error>
{
    match *route {
        ApiRoute::Config => {
            respond(res, format, &context.config.read()
                .expect("live configuration")
                .to_json())
        }
        ApiRoute::Peers => {
            respond(res, format, &context.schedule.get_peers().as_ref()
                .map(|x| &x.peers))
        }
    }
}

impl HttpContext for Context {
    // Defaults for now
}


fn serve_error_page(code: u32, response: &mut Response) {
    let (status, reason) = match code {
        400 => (400, "Bad Request"),
        404 => (404, "Not Found"),
        413 => (413, "Payload Too Large"),
        408 => (408, "Request Timeout"),
        431 => (431, "Request Header Fields Too Large"),
        500 => (500, "InternalServerError"),
        _ => unreachable!(),
    };
    response.status(status, reason);
    let data = format!("<h1>{} {}</h1>\n\
        <p><small>Served for you by rotor-http</small></p>\n",
        status, reason);
    let bytes = data.as_bytes();
    response.add_length(bytes.len() as u64).unwrap();
    response.add_header("Content-Type", b"text/html").unwrap();
    response.done_headers().unwrap();
    response.write_body(bytes);
    response.done();
}

impl Server for Public {
    type Context = Context;
    fn headers_received(head: Head, res: &mut Response,
        scope: &mut Scope<Context>)
        -> Option<(Self, RecvMode, Time)>
    {
        use self::Route::*;
        if !head.path.starts_with('/') {
            // Don't know what to do with that ugly urls
            return None;
        }
        let path = match head.path.find('?') {
            Some(x) => &head.path[..x],
            None => head.path,
        };
        let route = match path_component(&path[..]) {
            ("", _) => Some(Index),
            ("js", _) | ("css", _) => Some(Static(path.to_string())),
            ("v1", suffix) => parse_api(suffix),
            (_, _) => None,
        };
        debug!("Routed {:?} to {:?}", head, route);
        match route {
            Some(route) => {
                Some((Public(route), RecvMode::Buffered(1024),
                    scope.now() + Duration::new(120, 0)))
            }
            None => {
                serve_error_page(500, res);
                None
            }
        }
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
                serve_error_page(404, res);
            }
            Err(ref e) if e.kind() == io::ErrorKind::PermissionDenied => {
                serve_error_page(403, res);
            }
            Err(e) => {
                info!("Error serving {:?}: {}", self.0, e);
                serve_error_page(500, res);
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
        -> Option<(Self, Time)>
    {
        unimplemented!();
    }
    fn wakeup(self, _response: &mut Response, _scope: &mut Scope<Context>)
        -> Option<Self>
    {
        unimplemented!();
    }
}
