use std::io::{self, Read, Seek};
use std::fs::{File, metadata};
use std::path::{PathBuf};

use futures::future::{ok, Future};
use tk_http::Status;
use tk_http::server::{Codec as CodecTrait};
use tk_http::server::{EncoderDone, Error};

use frontend::reply;
use frontend::routing::{LogRoute, Range};
use routing_util::path_component;
use shared::SharedState;


const MAX_LOG_RESPONSE: u64 = 1048576;


pub type Request<S> = Box<CodecTrait<S, ResponseFuture=Reply<S>>>;
pub type Reply<S> = Box<Future<Item=EncoderDone<S>, Error=Error>>;

quick_error! {
    #[derive(Debug)]
    pub enum FileError {
        RequestRange {
            description("request range is invalid")
        }
        Io(path: PathBuf, err: io::Error) {
            display("can't open file {:?}: {}", path, err)
            description("can't open static file to serve")
        }
    }
}


pub fn serve<S: 'static>(state: &SharedState, route: &LogRoute)
    -> Result<Request<S>, Error>
{
    use self::LogRoute::*;
    use self::Range::*;
    let (path, rng) = match *route {
        Index(ref tail, rng) => {
            let path = state.options.log_dir.join(".index").join(tail);
            (path, rng)
        }
        Global(ref tail, rng) => {
            let path = state.options.log_dir.join(".global").join(tail);
            (path, rng)
        }
        Changes(ref tail, rng) => {
            let path = state.options.log_dir.join(".changes").join(tail);
            (path, rng)
        }
        Role(ref tail, rng) => {
            let path = state.options.log_dir.join(tail);
            (path, rng)
        }
        External(ref tail, rng) => {
            let (name, suffix) = path_component(tail);
            let path = match state.sandbox.log_dirs.get(name) {
                Some(path) => path.join(suffix),
                None => return Err(Error::custom(
                    io::Error::new(io::ErrorKind::NotFound,
                    "directory not found in sandbox"))),
            };
            (path, rng)
        }
    };

    let mut file = File::open(&path)
        .map_err(|e| Error::custom(FileError::Io(path.clone(), e)))?;
    let meta = metadata(&path)
        .map_err(|e| Error::custom(FileError::Io(path.clone(), e)))?;

    let (start, end) = match rng {
        FromTo(x, y) => (x, y + 1),
        AllFrom(x) => (x, meta.len()),
        Last(x) => (meta.len().saturating_sub(x), meta.len()),
    };
    let num_bytes = match end.checked_sub(start) {
        Some(n) => n,
        None => {
            return Err(Error::custom(FileError::RequestRange));
        }
    };

    if num_bytes > MAX_LOG_RESPONSE {
        return Err(Error::custom(FileError::RequestRange));
    }

    let mut buf = vec![0u8; num_bytes as usize];
    if start > 0 {
        file.seek(io::SeekFrom::Start(start))
            .map_err(|e| Error::custom(FileError::Io(path.clone(), e)))?;
    }
    file.read(&mut buf)
        .map_err(|e| Error::custom(FileError::Io(path.clone(), e)))?;

    Ok(reply(move |mut e| {
        e.status(Status::PartialContent);
        e.add_length(num_bytes).unwrap();
        e.add_header("Content-Range",
            format!("bytes {}-{}/{}", start, end-1, meta.len()).as_bytes()
        ).unwrap();
        e.done_headers().unwrap();
        e.write_body(&buf);
        Box::new(ok(e.done()))
    }))
}
