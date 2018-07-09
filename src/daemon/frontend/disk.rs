use std::path::{PathBuf, Path};
use std::sync::Arc;

use capturing_glob::{glob_with, MatchOptions};
use futures::{Future, Async};
use futures::future::{ok, FutureResult, Either, loop_fn, Loop};
use futures_cpupool::{CpuPool, CpuFuture};
use tokio_io::AsyncWrite;
use tk_http::server;
use tk_http::Status;
use http_file_headers::{Input, Output, Config};

use frontend::Request;
use frontend::routing::Format;
use frontend::quick_reply::{reply};
use frontend::api::{respond};

lazy_static! {
    static ref POOL: CpuPool = CpuPool::new(8);
    static ref CONFIG: Arc<Config> = Config::new()
        .add_index_file("index.html")
        .done();
    static ref LOG_CONFIG: Arc<Config> = Config::new()
        .no_encodings()
        .content_type(false)  // so we don't serve logs as JS or HTML
        .done();
    static ref BACKUPS_CONFIG: Arc<Config> = Config::new()
        .no_encodings()       // always encoded
        .content_type(false)  // we replace content type
        .done();
    static ref WASM: Arc<Config> = Config::new()
        .no_encodings()       // never encode, so never out of sync
        .done();
}

type ResponseFuture<S> = Box<Future<Item=server::EncoderDone<S>,
                                   Error=server::Error>>;

struct Codec {
    fut: Option<CpuFuture<Output, Status>>,
    kind: Kind,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Asset,
    Log,
    Backup,
}

fn common_headers<S>(e: &mut server::Encoder<S>, kind: Kind) {
    e.format_header("Server",
        format_args!("verwalter/{}", env!("CARGO_PKG_VERSION"))).unwrap();
    match kind {
        Kind::Backup => {
            e.add_header("Content-Encoding", "gzip").unwrap();
            e.add_header("Content-Type", "application/json").unwrap();
        }
        _ => {}
    }
}

fn respond_error<S: 'static>(status: Status, mut e: server::Encoder<S>)
    -> FutureResult<server::EncoderDone<S>, server::Error>
{
    let body = format!("{} {}", status.code(), status.reason());
    e.status(status);
    e.add_length(body.as_bytes().len() as u64).unwrap();
    common_headers(&mut e, Kind::Asset);
    if e.done_headers().unwrap() {
        e.write_body(body.as_bytes());
    }
    ok(e.done())
}

impl<S: AsyncWrite + Send + 'static> server::Codec<S> for Codec {
    type ResponseFuture = ResponseFuture<S>;
    fn recv_mode(&mut self) -> server::RecvMode {
        server::RecvMode::buffered_upfront(0)
    }
    fn data_received(&mut self, data: &[u8], end: bool)
        -> Result<Async<usize>, server::Error>
    {
        debug_assert!(end && data.len() == 0);
        Ok(Async::Ready(0))
    }
    fn start_response(&mut self, mut e: server::Encoder<S>)
        -> Self::ResponseFuture
    {
        let kind = self.kind;
        Box::new(self.fut.take().unwrap().then(move |result| {
            match result {
                Ok(Output::File(outf)) | Ok(Output::FileRange(outf)) => {
                    if outf.is_partial() {
                        e.status(Status::PartialContent);
                    } else {
                        e.status(Status::Ok);
                    }
                    e.add_length(outf.content_length()).unwrap();
                    common_headers(&mut e, kind);
                    for (name, val) in outf.headers() {
                        e.format_header(name, val).unwrap();
                    }
                    // add headers
                    if e.done_headers().unwrap() {
                        // start writing body
                        Either::B(loop_fn((e, outf), |(mut e, mut outf)| {
                            POOL.spawn_fn(move || {
                                outf.read_chunk(&mut e).map(|b| (b, e, outf))
                            }).and_then(|(b, e, outf)| {
                                e.wait_flush(4096).map(move |e| (b, e, outf))
                            }).map(|(b, e, outf)| {
                                if b == 0 {
                                    Loop::Break(e.done())
                                } else {
                                    Loop::Continue((e, outf))
                                }
                            }).map_err(|e| server::Error::custom(e))
                        }))
                    } else {
                        Either::A(ok(e.done()))
                    }
                }
                Ok(Output::FileHead(head)) | Ok(Output::NotModified(head)) => {
                    if head.is_not_modified() {
                        e.status(Status::NotModified);
                    } else if head.is_partial() {
                        e.status(Status::PartialContent);
                        e.add_length(head.content_length()).unwrap();
                    } else {
                        e.status(Status::Ok);
                        e.add_length(head.content_length()).unwrap();
                    }
                    common_headers(&mut e, kind);
                    for (name, val) in head.headers() {
                        e.format_header(name, val).unwrap();
                    }
                    assert_eq!(e.done_headers().unwrap(), false);
                    Either::A(ok(e.done()))
                }
                Ok(Output::InvalidRange) => {
                    Either::A(respond_error(
                        Status::RequestRangeNotSatisfiable, e))
                }
                Ok(Output::InvalidMethod) => {
                    Either::A(respond_error(
                        Status::MethodNotAllowed, e))
                }
                Ok(Output::NotFound) | Ok(Output::Directory) => {
                    Either::A(respond_error(Status::NotFound, e))
                }
                Err(status) => {
                    Either::A(respond_error(status, e))
                }
            }
        }))
    }
}

pub fn index_response<S>(head: &server::Head, base: &Path, frontend: &str)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let inp = Input::from_headers(&*CONFIG, head.method(), head.headers());
    let path = base.join(frontend).join("index.html");
    let fut = POOL.spawn_fn(move || {
        inp.probe_file(&path).map_err(|e| {
            error!("Error reading file {:?}: {}", path, e);
            Status::InternalServerError
        })
    });
    Ok(Box::new(Codec {
        fut: Some(fut),
        kind: Kind::Asset,
    }) as Request<S>)
}

pub fn common_response<S>(head: &server::Head, path: String,
    base: &Path)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let inp = Input::from_headers(&*CONFIG, head.method(), head.headers());
    // path is validated for ".." and root in routing
    let path = base.join("common").join(path);
    let fut = POOL.spawn_fn(move || {
        inp.probe_file(&path).map_err(|e| {
            error!("Error reading file {:?}: {}", path, e);
            Status::InternalServerError
        })
    });
    Ok(Box::new(Codec {
        fut: Some(fut),
        kind: Kind::Asset,
    }))
}

pub fn alter_index_response<S>(head: &server::Head, dir: String,
    base: &PathBuf)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let inp = Input::from_headers(&*CONFIG, head.method(), head.headers());
    let path = base.join(dir).join("index.html");
    let fut = POOL.spawn_fn(move || {
        inp.probe_file(&path).map_err(|e| {
            error!("Error reading file {:?}: {}", path, e);
            Status::InternalServerError
        })
    });
    Ok(Box::new(Codec {
        fut: Some(fut),
        kind: Kind::Asset,
    }) as Request<S>)
}

pub fn alter_static_response<S>(head: &server::Head, path: String, base: &Path)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let inp = Input::from_headers(&*CONFIG, head.method(), head.headers());
    // path is validated for ".." and root in routing
    let path = base.join(path);
    let fut = POOL.spawn_fn(move || {
        inp.probe_file(&path).map_err(|e| {
            error!("Error reading file {:?}: {}", path, e);
            Status::InternalServerError
        })
    });
    Ok(Box::new(Codec {
        fut: Some(fut),
        kind: Kind::Asset,
    }))
}

pub fn log_response<S>(head: &server::Head, full_path: PathBuf)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let inp = Input::from_headers(&*LOG_CONFIG, head.method(), head.headers());
    // path is validated for ".." in routing
    let fut = POOL.spawn_fn(move || {
        inp.probe_file(&full_path).map_err(|e| {
            error!("Error reading log file {:?}: {}", full_path, e);
            Status::InternalServerError
        })
    });
    Ok(Box::new(Codec {
        fut: Some(fut),
        kind: Kind::Log,
    }))
}

pub fn list_backups<S>(schedule_dir: &Path, format: Format)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let dir = schedule_dir.to_str()
        .expect("schedule_dir is utf-8")
        .to_string();
    Ok(reply(move |e| {
        Box::new(POOL.spawn_fn(move || {
            let items = glob_with(
                &format!("{}/(*-*).json.gz", dir), &MatchOptions {
                    case_sensitive: true,
                    require_literal_separator: true,
                    require_literal_leading_dot: true,
                })
                .map(|items| {
                    items.filter_map(|e| {
                        e.map_err(|e| {
                            error!("Error listing backups: {}", e);
                        }).ok()
                    })
                    .filter_map(|e| {
                        e.group(1)
                        .and_then(|x| x.to_str())
                        .map(|x| x.to_string())
                    })
                    .collect::<Vec<_>>()
                })
                .unwrap_or_else(|e| {
                    error!("Error listing backups: {}", e);
                    Vec::new()
                });
            Ok(items)
        })
        .and_then(move |items| respond(e, format, items)))
    }))
}

pub fn serve_backup<S>(name: String, head: &server::Head, schedule_dir: &Path)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let inp = Input::from_headers(&*BACKUPS_CONFIG,
        head.method(), head.headers());
    let path = schedule_dir.join(&format!("{}.json.gz", name));
    let fut = POOL.spawn_fn(move || {
        inp.probe_file(&path).map_err(|e| {
            error!("Error reading file {:?}: {}", path, e);
            Status::InternalServerError
        })
    });
    Ok(Box::new(Codec {
        fut: Some(fut),
        kind: Kind::Backup,
    }) as Request<S>)
}

pub fn serve_wasm<S>(head: &server::Head, file: PathBuf)
    -> Result<Request<S>, server::Error>
    where S: AsyncWrite + Send + 'static
{
    let inp = Input::from_headers(&*CONFIG, head.method(), head.headers());
    let fut = POOL.spawn_fn(move || {
        inp.probe_file(&file).map_err(|e| {
            error!("Error reading wasm {:?}: {}", file, e);
            Status::InternalServerError
        })
    });
    Ok(Box::new(Codec {
        fut: Some(fut),
        kind: Kind::Backup,
    }) as Request<S>)
}
