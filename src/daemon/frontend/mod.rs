use std::ascii::AsciiExt;
use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::fs::{File, metadata};
use std::io::{self, Read, Write, Seek};
use std::path::Component::Normal;
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use std::sync::Arc;
use std::time::{Duration};

use futures::{Future, Async};
use gron::json_to_gron;
use rustc_serialize::Encodable;
use rustc_serialize::json::{as_json, as_pretty_json, Json};
use tk_http::Status;
use tk_http::server::{Codec as CodecTrait, Dispatcher as DispatcherTrait};
use tk_http::server::{Head, Encoder, EncoderDone, RecvMode, Error};
use tokio_io::AsyncWrite;

use id::Id;
use elect::Epoch;
use shared::{SharedState, PushActionError};
use time_util::ToMsec;

mod api;
mod log;
mod disk;
mod error_page;
mod quick_reply;
mod routing;
pub mod serialize;
mod to_json;

use frontend::to_json::ToJson;
use frontend::routing::{route, Route};
pub use frontend::quick_reply::{reply, read_json};
pub use frontend::error_page::serve_error_page;


pub type Request<S> = Box<CodecTrait<S, ResponseFuture=Reply<S>>>;
pub type Reply<S> = Box<Future<Item=EncoderDone<S>, Error=Error>>;


pub struct Dispatcher {
    pub state: SharedState,
    pub dir: Arc<PathBuf>,
}

impl<S: AsyncWrite + Send + 'static> DispatcherTrait<S> for Dispatcher {
    type Codec = Request<S>;
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        use self::Route::*;
        match route(headers) {
            Index => {
                disk::index_response(headers, &self.dir)
            }
            Static(path) => {
                disk::common_response(headers, path, &self.dir)
            }
            Api(ref route, fmt) => {
                api::serve(&self.state, route, fmt)
            }
            Log(ref route) => {
                log::serve(&self.state, route)
            }
            NotFound => {
                serve_error_page(Status::NotFound)
            }
        }
    }
}
