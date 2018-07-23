use std::error::Error;
use std::str::FromStr;
use std::net::SocketAddr;

use tk_cantal;
use tk_easyloop;

use futures::{Future, Stream};
use futures::future::ok;
use shared::SharedState;
use peer::Peer;
use id::Id;
use elect::peers_refresh;


pub fn spawn_fetcher(state: &SharedState, port: u16)
    -> Result<(), Box<Error>>
{
    let state = state.clone();
    let conn = tk_cantal::connect_local(&tk_easyloop::handle());
    tk_easyloop::spawn(
        tk_easyloop::interval(peers_refresh())
        .map_err(|_| { unreachable!() })
        .for_each(move |_| {
            let state = state.clone();
            conn.get_peers().then(move |res| {
                match res {
                    Ok(peers) => {
                        state.set_peers(peers.requested, peers.peers.into_iter()
                            .filter_map(|p| {
                                let id = Id::from_str(&p.id);
                                id.ok().map(move |id| (id.clone(), Peer {
                                    id,
                                    addr: p.primary_addr
                                        .and_then(|x| x.parse::<SocketAddr>().ok())
                                        .map(|x| SocketAddr::new(x.ip(), port)),
                                    name: p.name,
                                    hostname: p.hostname,
                                    schedule: None,
                                    known_since: p.known_since,
                                    last_report_direct: p.last_report_direct,
                                    errors: 0,
                                }))
                            }).collect());
                    }
                    Err(e) => {
                        error!("Error fetching cantal data: {}", e);
                    }
                }
                ok(())
            })
        })
        .then(|_| -> Result<(), ()> { panic!("cantal loop exits") }));
    Ok(())
}
