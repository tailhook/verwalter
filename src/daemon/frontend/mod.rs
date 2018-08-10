use std::path::{PathBuf};
use std::sync::Arc;
use std::net::SocketAddr;

use futures::{Future};
use tk_http::Status;
use tk_http::server::{Codec as CodecTrait, Dispatcher as DispatcherTrait};
use tk_http::server::{Head, EncoderDone, Error};
use tokio_io::{AsyncRead, AsyncWrite};

use shared::{SharedState};

mod api;
mod log;
mod disk;
mod error_page;
mod quick_reply;
mod routing;
pub mod serialize;
mod to_json;
pub mod graphql;
mod websocket;
pub mod incoming;
mod dispatcher;
pub mod channel;

mod status;

use frontend::routing::{route, Route};
pub use frontend::incoming::Subscription;
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
    pub ip: SocketAddr,
    pub state: SharedState,
    pub config: Arc<Config>,
    pub incoming: incoming::Incoming,
}

impl<S> DispatcherTrait<S> for Dispatcher
    where S: AsyncRead + AsyncWrite + Send + 'static
{
    type Codec = Request<S>;
    fn headers_received(&mut self, headers: &Head)
        -> Result<Self::Codec, Error>
    {
        use self::Route::*;
        use frontend::routing::ApiRoute::{Backup, Backups, Graphql, GraphqlWs};
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
            Api(Graphql, fmt) => {
                graphql::serve(&self.state, &self.config, fmt)
            }
            Api(GraphqlWs(ws), _fmt) => {
                Ok(websocket::serve(ws,
                    &self.incoming, &self.state, &self.config))
            }
            Api(ref route, fmt) => {
                api::serve(&self.state, &self.config, route, fmt, self.ip)
            }
            Log(ref route) => {
                log::serve(headers, &self.state, route)
            }
            WasmScheduler => {
                let path = self.state.options.config_dir
                    .join("scheduler/v1/scheduler.wasm");
                disk::serve_wasm(headers, path)
            }
            WasmQuery => {
                let path = self.state.options.config_dir
                    .join("scheduler/v1/query.wasm");
                disk::serve_wasm(headers, path)
            }
            NotFound => {
                serve_error_page(Status::NotFound)
            }
            BadContentType => {
                serve_error_page(Status::UnsupportedMediaType)
            }
            BadRequest => {
                serve_error_page(Status::BadRequest)
            }
        }
    }
}
