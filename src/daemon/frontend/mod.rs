use std::path::{PathBuf};
use std::sync::Arc;

use futures::{Future};
use tk_http::Status;
use tk_http::server::{Codec as CodecTrait, Dispatcher as DispatcherTrait};
use tk_http::server::{Head, EncoderDone, Error};
use tokio_io::AsyncWrite;

use shared::{SharedState};

mod api;
mod log;
mod disk;
mod error_page;
mod quick_reply;
mod routing;
pub mod serialize;
mod to_json;

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
