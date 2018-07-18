use std::time::Duration;
use std::process::exit;
use std::path::Path;
use std::sync::Arc;

use ns_router::Router as NsRouter;
use futures::{Future, Stream};
use tk_easyloop::{handle, spawn};
use tk_http;
use tk_http::server::{Proto};
use tokio_core::net::TcpListener;
use tk_listen::ListenExt;

use frontend;
use frontend::graphql;
use shared::SharedState;


pub fn spawn_listener(ns: &NsRouter, addr: &str,
    state: &SharedState, rx: frontend::channel::Receiver, static_dir: &Path,
    default_frontend: &str,
    schedule_dir: &Path)
    -> Result<(), Box<::std::error::Error>>
{
    let str_addr = addr.to_string();
    let state = state.clone();
    let config = Arc::new(frontend::Config {
        dir: static_dir.to_path_buf(),
        default_frontend: default_frontend.to_string(),
        schedule_dir: schedule_dir.to_path_buf(),
    });
    let ctx = graphql::Context {
        state: state.clone(),
        config: config.clone(),
    };
    let incoming = frontend::incoming::Incoming::new(&ctx);
    rx.start(&incoming);
    let hcfg = tk_http::server::Config::new()
        .inflight_request_limit(2)
        .inflight_request_prealoc(0)
        .first_byte_timeout(Duration::new(10, 0))
        .keep_alive_timeout(Duration::new(600, 0))
        .headers_timeout(Duration::new(1, 0))             // no big headers
        .input_body_byte_timeout(Duration::new(1, 0))     // no big bodies
        .input_body_whole_timeout(Duration::new(2, 0))
        .output_body_byte_timeout(Duration::new(1, 0))
        // sometimes we download log files,
        // still we shouldn't allow to DoS too much
        .output_body_whole_timeout(Duration::new(120, 0))
        .done();

    spawn(ns.resolve_auto(addr, 8379).map(move |addresses| {
        for addr in addresses.at(0).addresses() {
            info!("Listening on {}", addr);
            let listener = TcpListener::bind(&addr, &handle())
                .unwrap_or_else(|e| {
                    error!("Can't bind {}: {}", addr, e);
                    exit(81);
                });
            let hcfg = hcfg.clone();
            let state = state.clone();
            let config = config.clone();
            let incoming = incoming.clone();
            spawn(listener.incoming()
                .sleep_on_error(Duration::from_millis(100), &handle())
                .map(move |(socket, saddr)| {
                    Proto::new(socket, &hcfg,
                        frontend::Dispatcher {
                            incoming: incoming.clone(),
                            state: state.clone(),
                            config: config.clone(),
                        },
                        &handle())
                    .map_err(move |e| {
                        debug!("Http protocol error for {}: {}", saddr, e);
                    })
                })
                .listen(500)
                .then(move |res| -> Result<(), ()> {
                    error!("Listener {} exited: {:?}", addr, res);
                    exit(81);
                }));
        }
    }).map_err(move |e| {
        error!("Can't bind address {}: {}", str_addr, e);
        exit(3);
    }));
    Ok(())
}
