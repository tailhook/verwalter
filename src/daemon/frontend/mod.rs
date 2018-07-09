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


pub struct Config {
    pub dir: PathBuf,
    pub default_frontend: String,
    pub schedule_dir: PathBuf,
}

pub struct Dispatcher {
    pub state: SharedState,
    pub config: Arc<Config>,
}

impl<S: AsyncWrite + Send + 'static> DispatcherTrait<S> for Dispatcher {
    type Codec = Request<S>;
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        use self::Route::*;
        use frontend::routing::ApiRoute::{Backup, Backups};
        match route(headers) {
            CommonIndex => {
                disk::index_response(headers, &self.config.dir,
                    &self.config.default_frontend)
            }
            CommonStatic(path) => {
                disk::common_response(headers, path, &self.config.dir)
            }
            AlterIndex(dir) => {
                disk::alter_index_response(headers, dir, &self.config.dir)
            }
            AlterStatic(path) => {
                disk::alter_static_response(headers, path, &self.config.dir)
            }
            Api(Backups, fmt) => {
                disk::list_backups(&self.config.schedule_dir, fmt)
            }
            Api(Backup(name), _) => {
                disk::serve_backup(name, headers, &self.config.schedule_dir)
            }
            Api(ref route, fmt) => {
                api::serve(&self.state, &self.config, route, fmt)
            }
            Log(ref route) => {
                log::serve(headers, &self.state, route)
            }
            NotFound => {
                serve_error_page(Status::NotFound)
            }
            BadContentType => {
                serve_error_page(Status::UnsupportedMediaType)
            }
        }
    }
}
