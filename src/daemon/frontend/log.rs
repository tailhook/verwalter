use std::io;

use futures::future::{Future};
use tokio_io::AsyncWrite;
use tk_http::server::{Codec as CodecTrait};
use tk_http::server::{self, EncoderDone, Error};

use frontend::routing::{LogRoute};
use routing_util::path_component;
use shared::SharedState;
use frontend::disk;


pub type Request<S> = Box<CodecTrait<S, ResponseFuture=Reply<S>>>;
pub type Reply<S> = Box<Future<Item=EncoderDone<S>, Error=Error>>;


pub fn serve<S>(head: &server::Head, state: &SharedState, route: &LogRoute)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    use self::LogRoute::*;
    // path is validated for `../` in routing
    let path = match *route {
        Index(ref tail) => state.options.log_dir.join(".index").join(tail),
        Global(ref tail) => state.options.log_dir.join(".global").join(tail),
        Changes(ref tail) => state.options.log_dir.join(".changes").join(tail),
        Role(ref tail) => state.options.log_dir.join(tail),
        External(ref tail) => {
            let (name, suffix) = path_component(tail);
            match state.sandbox.log_dirs.get(name) {
                Some(path) => path.join(suffix),
                None => return Err(Error::custom(
                    io::Error::new(io::ErrorKind::NotFound,
                    "directory not found in sandbox"))),
            }
        }
    };
    return disk::log_response(head, path);
}
