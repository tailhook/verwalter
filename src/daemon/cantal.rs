use std::error::Error;
use std::str::FromStr;
use std::time::{SystemTime, Duration};
use std::net::SocketAddr;

use tk_http;
use tk_cantal;
use tk_easyloop;

use futures::{Future, Stream};
use futures::future::ok;
use shared::SharedState;
use peer::Peer;
use id::Id;


pub fn spawn_fetcher(state: &SharedState, port: u16)
    -> Result<(), Box<Error>>
{
    let state = state.clone();
    let conn = tk_cantal::connect_local(&tk_easyloop::handle());
    tk_easyloop::spawn(
        tk_easyloop::interval(Duration::new(3, 0))
        .map_err(|_| { unreachable!() })
        .for_each(move |_| {
            let state = state.clone();
            let time = SystemTime::now();
            conn.get_peers()
            .and_then(move |peers| {
                state.set_peers(peers.requested, peers.peers.into_iter()
                    .filter_map(|p| {
                        let id = Id::from_str(&p.id);
                        id.ok().map(move |id| (id, Peer {
                            addr: p.primary_addr
                                .and_then(|x| x.parse::<SocketAddr>().ok())
                                .map(|x| SocketAddr::new(x.ip(), port)),
                            name: p.name,
                            hostname: p.hostname,
                            // last_report: None, // TODO(tailhook)
                        }))
                    }).collect());
                ok(())
            })
            .map_err(|e| {
                error!("Error fetching cantal data: {}", e);
            })
        }));
    Ok(())
}
