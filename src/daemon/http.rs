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
use shared::SharedState;


pub fn spawn_listener(ns: &NsRouter, addr: &str,
    state: &SharedState, static_dir: &Path, schedule_dir: &Path)
    -> Result<(), Box<::std::error::Error>>
{
    let str_addr = addr.to_string();
    let state = state.clone();
    let static_dir = Arc::new(static_dir.to_path_buf());
    let schedule_dir = Arc::new(schedule_dir.to_path_buf());
    let hcfg = tk_http::server::Config::new()
        .inflight_request_limit(2)
        .inflight_request_prealoc(0)
        .first_byte_timeout(Duration::new(10, 0))
        .keep_alive_timeout(Duration::new(600, 0))
        .headers_timeout(Duration::new(1, 0))             // no big headers
        .input_body_byte_timeout(Duration::new(1, 0))     // no big bodies
        .input_body_whole_timeout(Duration::new(2, 0))
        .output_body_byte_timeout(Duration::new(1, 0))
        .output_body_whole_timeout(Duration::new(10, 0))  // max 65k bytes
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
            let static_dir = static_dir.clone();
            let schedule_dir = schedule_dir.clone();
            spawn(listener.incoming()
                .sleep_on_error(Duration::from_millis(100), &handle())
                .map(move |(socket, saddr)| {
                    Proto::new(socket, &hcfg,
                        frontend::Dispatcher {
                            state: state.clone(),
                            dir: static_dir.clone(),
                            schedule_dir: schedule_dir.clone(),
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
