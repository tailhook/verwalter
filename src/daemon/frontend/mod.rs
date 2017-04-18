use std::io::{self, Read, Write, Seek};
use std::str::from_utf8;
use std::path::Path;
use std::path::Component::Normal;
use std::fs::{File, metadata};
use std::time::{Duration};
use std::ascii::AsciiExt;
use std::collections::{HashMap, HashSet};

use futures::{Future, Async};
use gron::json_to_gron;
use rustc_serialize::Encodable;
use rustc_serialize::json::{as_json, as_pretty_json, Json};
use tk_http::Status;
use tk_http::server::{Codec as CodecTrait, Dispatcher as DispatcherTrait};
use tk_http::server::{Head, Encoder, EncoderDone, RecvMode, Error};

use id::Id;
use elect::Epoch;
use shared::{SharedState, PushActionError};
use time_util::ToMsec;

mod api;
mod error_page;
mod files;
mod quick_reply;
mod routing;
mod serialize;
mod to_json;

use frontend::to_json::ToJson;
use frontend::routing::{route, Route};
pub use frontend::quick_reply::reply;
pub use frontend::error_page::serve_error_page;

const MAX_LOG_RESPONSE: u64 = 1048576;

pub type Request<S> = Box<CodecTrait<S, ResponseFuture=Reply<S>>>;
pub type Reply<S> = Box<Future<Item=EncoderDone<S>, Error=Error>>;


pub struct Dispatcher(pub SharedState);


/*

fn serve_log(route: &LogRoute, ctx: &Context, res: &mut Response)
    -> io::Result<()>
{
    use self::LogRoute::*;
    use self::Range::*;
    let (path, rng) = match *route {
        Index(ref tail, rng) => {
            let path = ctx.log_dir.join(".index").join(tail);
            (path, rng)
        }
        Global(ref tail, rng) => {
            let path = ctx.log_dir.join(".global").join(tail);
            (path, rng)
        }
        Changes(ref tail, rng) => {
            let path = ctx.log_dir.join(".changes").join(tail);
            (path, rng)
        }
        Role(ref tail, rng) => {
            let path = ctx.log_dir.join(tail);
            (path, rng)
        }
        External(ref tail, rng) => {
            let (name, suffix) = path_component(tail);
            let path = match ctx.sandbox.log_dirs.get(name) {
                Some(path) => path.join(suffix),
                None => return Err(io::Error::new(io::ErrorKind::NotFound,
                    "directory not found in sandbox")),
            };
            (path, rng)
        }
    };
    let mut file = try!(File::open(&path));
    let meta = try!(metadata(&path));

    let (start, end) = match rng {
        FromTo(x, y) => (x, y + 1),
        AllFrom(x) => (x, meta.len()),
        Last(x) => (meta.len().saturating_sub(x), meta.len()),
    };
    let num_bytes = match end.checked_sub(start) {
        Some(n) => n,
        None => {
            return Err(io::Error::new(io::ErrorKind::InvalidData,
                "Request range is invalid"));
        }
    };

    if num_bytes > MAX_LOG_RESPONSE {
        return Err(io::Error::new(io::ErrorKind::InvalidData,
            "Requested range is too large"));
    }

    let mut buf = vec![0u8; num_bytes as usize];
    if start > 0 {
        try!(file.seek(io::SeekFrom::Start(start)));
    }
    try!(file.read(&mut buf));

    res.status(206, "OK");
    res.add_length(num_bytes).unwrap();
    res.add_header("Content-Range",
        format!("bytes {}-{}/{}", start, end-1, meta.len()).as_bytes()
    ).unwrap();
    res.done_headers().unwrap();
    res.write_body(&buf);
    res.done();
    Ok(())
}
*/

impl<S: 'static> DispatcherTrait<S> for Dispatcher {
    type Codec = Request<S>;
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        use self::Route::*;
        match route(headers) {
            Index => {
                files::INDEX.serve()
            }
            Static(ref file) => {
                file.serve()
            }
            Api(ref route, fmt) => {
                api::serve(&self.0, route, fmt)
            }
            Log(ref x) => {
                serve_error_page(Status::NotImplemented)
                // unimplemented!();
                // serve_log(x, scope, res)
            }
            NotFound => {
                serve_error_page(Status::NotFound)
            }
        }
    }
}
