use std::io::BufWriter;
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use futures::future::{FutureResult, ok, Future};
use gron::json_to_gron;
use serde::Serialize;
use serde_json::{Value, to_writer, to_writer_pretty, to_value};
use tk_http::Status::{self, NotImplemented};
use tk_http::server::{Codec as CodecTrait, Dispatcher as DispatcherTrait};
use tk_http::server::{Head, Encoder, EncoderDone, RecvMode, Error};

use id::Id;
use elect::Epoch;
use shared::SharedState;
use frontend::reply;
use frontend::to_json::ToJson;
use frontend::routing::{ApiRoute, Format};
use frontend::serialize::serialize_opt_timestamp;
use frontend::error_page::serve_error_page;


pub type Request<S> = Box<CodecTrait<S, ResponseFuture=Reply<S>>>;
pub type Reply<S> = Box<Future<Item=EncoderDone<S>, Error=Error>>;


fn get_metrics() -> HashMap<&'static str, Value>
{
    use scheduler::main as S;
    use elect::machine as M;
    //use elect::network as N;
    vec![
        ("scheduling_time", S::SCHEDULING_TIME.js()),
        ("scheduler_succeeded", S::SCHEDULER_SUCCEEDED.js()),
        ("scheduler_failed", S::SCHEDULER_FAILED.js()),

        ("start_election_no", M::START_ELECTION_NO.js()),
        ("start_election_tm", M::START_ELECTION_TM.js()),
        ("ping_all_no", M::PING_ALL_NO.js()),
        ("ping_all_tm", M::PING_ALL_TM.js()),
        ("outdated_no", M::OUTDATED_NO.js()),
        ("outdated_tm", M::OUTDATED_TM.js()),
        ("ping_no", M::PING_NO.js()),
        ("ping_tm", M::PING_TM.js()),
        ("pong_no", M::PONG_NO.js()),
        ("pong_tm", M::PONG_TM.js()),
        ("vote_confirm_no", M::VOTE_CONFIRM_NO.js()),
        ("vote_confirm_tm", M::VOTE_CONFIRM_TM.js()),
        ("became_leader_no", M::BECAME_LEADER_NO.js()),
        ("became_leader_tm", M::BECAME_LEADER_TM.js()),
        ("vote_for_me_no", M::VOTE_FOR_ME_NO.js()),
        ("vote_for_me_tm", M::VOTE_FOR_ME_TM.js()),
        ("vote_other_no", M::VOTE_OTHER_NO.js()),
        ("vote_other_tm", M::VOTE_OTHER_TM.js()),
        ("late_vote_no", M::LATE_VOTE_NO.js()),
        ("late_vote_tm", M::LATE_VOTE_TM.js()),
        ("newer_ping_no", M::NEWER_PING_NO.js()),
        ("newer_ping_tm", M::NEWER_PING_TM.js()),
        ("new_vote_no", M::NEW_VOTE_NO.js()),
        ("new_vote_tm", M::NEW_VOTE_TM.js()),
        ("bad_hosts_no", M::BAD_HOSTS_NO.js()),
        ("bad_hosts_tm", M::BAD_HOSTS_TM.js()),
        ("self_elect_no", M::SELF_ELECT_NO.js()),
        ("self_elect_tm", M::SELF_ELECT_TM.js()),

        ("elect_start_no", M::ELECT_START_NO.js()),
        ("elect_start_tm", M::ELECT_START_TM.js()),
        ("elect_timeo_no", M::ELECT_TIMEO_NO.js()),
        ("elect_timeo_tm", M::ELECT_TIMEO_TM.js()),
        ("elect_voted_no", M::ELECT_VOTED_NO.js()),
        ("elect_voted_tm", M::ELECT_VOTED_TM.js()),
        ("elect_unresponsive_no", M::ELECT_UNRESPONSIVE_NO.js()),
        ("elect_unresponsive_tm", M::ELECT_UNRESPONSIVE_TM.js()),
        ("elect_conflict_no", M::ELECT_CONFLICT_NO.js()),
        ("elect_conflict_tm", M::ELECT_CONFLICT_TM.js()),
        ("elect_unsolicit_pong_no", M::ELECT_UNSOLICIT_PONG_NO.js()),
        ("elect_unsolicit_pong_tm", M::ELECT_UNSOLICIT_PONG_TM.js()),
        ("elect_newer_pong_no", M::ELECT_NEWER_PONG_NO.js()),
        ("elect_newer_pong_tm", M::ELECT_NEWER_PONG_TM.js()),

        //("broadcasts_sent", N::BROADCASTS_SENT.js()),
        //("broadcasts_errored", N::BROADCASTS_ERRORED.js()),
        //("pongs_sent", N::PONGS_SENT.js()),
        //("pongs_errored", N::PONGS_ERRORED.js()),
        //("last_ping_all", N::LAST_PING_ALL.js()),
        //("last_vote", N::LAST_VOTE.js()),
        //("last_confirm_vote", N::LAST_CONFIRM_VOTE.js()),
        //("last_pong", N::LAST_PONG.js()),
    ].into_iter().collect()
}

pub fn respond<D: Serialize, S>(mut e: Encoder<S>, format: Format, data: D)
    -> FutureResult<EncoderDone<S>, Error>
{
    e.status(Status::Ok);
    e.add_chunked().unwrap();
    let ctype = match format {
        Format::Json => "application/json",
        Format::Gron => "text/x-gron",
        Format::Plain => "application/json",
    };
    e.add_header("Content-Type", ctype.as_bytes()).unwrap();
    if e.done_headers().unwrap() {
        match format {
            Format::Json => {
                to_writer(&mut BufWriter::new(&mut e), &data)
                    .expect("data is always serializable");
            }
            Format::Gron => {
                json_to_gron(&mut BufWriter::new(&mut e), "json",
                    &to_value(data).expect("data is always convertible"))
                    .expect("data is always serializable");
            }
            Format::Plain => {
                to_writer_pretty(&mut BufWriter::new(&mut e), &data)
                    .expect("data is always serializable");
            }
        };
    }
    ok(e.done())
}

pub fn serve<S: 'static>(state: &SharedState, route: &ApiRoute, format: Format)
    -> Result<Request<S>, Error>
{
    use self::ApiRoute::*;
    let state = state.clone();
    match *route {
        Status => {
            Ok(reply(move |e| {
                #[derive(Serialize)]
                struct LeaderInfo<'a> {
                    id: &'a Id,
                    name: &'a str,
                    hostname: &'a str,
                    addr: Option<String>,
                }
                #[derive(Serialize)]
                struct Status<'a> {
                    version: &'static str,
                    id: &'a Id,
                    name: &'a str,
                    hostname: &'a str,
                    peers: usize,
                    #[serde(serialize_with="serialize_opt_timestamp")]
                    peers_timestamp: Option<SystemTime>,
                    leader: Option<LeaderInfo<'a>>,
                    scheduler_state: &'static str,
                    roles: usize,
                    election_epoch: Epoch,
                    #[serde(serialize_with="serialize_opt_timestamp")]
                    last_stable_timestamp: Option<SystemTime>,
                    num_errors: usize,
                    errors: &'a HashMap<&'static str, String>,
                    failed_roles: &'a HashSet<String>,
                    debug_force_leader: bool,
                    //self_report: Option<self_meter::Report>,
                    //threads_report: HashMap<String, self_meter::ThreadReport>,
                    metrics: HashMap<&'static str, Value>,
                }
                let peers = state.peers();
                let election = state.election();
                let schedule = state.schedule();
                let leader = if election.is_leader {
                    Some(LeaderInfo {
                        id: state.id(),
                        name: &state.name,
                        hostname: &state.hostname,
                        // TODO(tailhook) resolve listening address and show
                        addr: None,
                    })
                } else {
                    election.leader.as_ref()
                    .and_then(|id| peers.1.get(id).map(|p| (id, p)))
                    .map(|(id, peer)| LeaderInfo {
                        id: id,
                        name: &peer.name,
                        hostname: &peer.hostname,
                        addr: peer.addr.map(|x| x.to_string()),
                    })
                };
                let errors = state.errors();
                let failed_roles = state.failed_roles();
                //let (me, thr) = {
                //    let meter = meter.lock().unwrap();
                //    (meter.report(),
                //     meter.thread_report()
                //        .map(|x| x.map(|(k, v)| (k.to_string(), v)).collect())
                //        .unwrap_or(HashMap::new()))
                //};
                Box::new(respond(e, format, &Status {
                    version: concat!("v", env!("CARGO_PKG_VERSION")),
                    id: &state.id,
                    name: &state.name,
                    hostname: &state.hostname,
                    peers: peers.1.len(),
                    peers_timestamp: Some(peers.0),
                    leader: leader,
                    roles: schedule.map(|x| x.num_roles).unwrap_or(0),
                    scheduler_state: state.scheduler_state().describe(),
                    election_epoch: election.epoch,
                    last_stable_timestamp: election.last_stable_timestamp,
                    num_errors: errors.len() + failed_roles.len(),
                    errors: &*errors,
                    failed_roles: &*failed_roles,
                    debug_force_leader: state.debug_force_leader(),
                    //self_report: me,
                    //threads_report: thr,
                    metrics: get_metrics(),
                }))
            }))
        }
        Peers => {
            #[derive(Serialize)]
            struct Peer<'a> {
                id: &'a Id,
                primary_addr: Option<String>,
                name: &'a String,
                hostname: &'a String,
            }
            Ok(reply(move |e| {
                Box::new(respond(e, format,
                    &state.peers().1.iter().map(|(id, peer)| Peer {
                        id: id,
                        name: &peer.name,
                        hostname: &peer.hostname,
                        primary_addr: peer.addr.map(|x| x.to_string()),
                    }).collect::<Vec<_>>()
                ))
            }))
        }
        Schedule => {
            serve_error_page(NotImplemented)
            //if let Some(schedule) = scope.state.schedule() {
            //    respond(res, format, &schedule)
            //} else {
            //    // TODO(tailhook) Should we return error code instead ?
            //    respond(res, format, Json::Null)
            //}
        }
        Scheduler => {
            serve_error_page(NotImplemented)
            //respond(res, format, &scope.state.scheduler_state())
        }
        SchedulerInput => {
            serve_error_page(NotImplemented)
            //respond(res, format, &scope.state.scheduler_debug_info().0)
        }
        SchedulerDebugInfo => {
            serve_error_page(NotImplemented)
            //respond_text(res, &scope.state.scheduler_debug_info().1)
        }
        Election => {
            Ok(reply(move |e| {
                Box::new(respond(e, format, &*state.election()))
            }))
        }
        PendingActions => {
            serve_error_page(NotImplemented)
            //respond(res, format, &scope.state.pending_actions())
        }
        ForceRenderAll => {
            serve_error_page(NotImplemented)
            //scope.state.force_render();
            //respond(res, format, "ok")
        }
        PushAction => {
            serve_error_page(NotImplemented)
            //let jdata = from_utf8(data).ok()
            //    .and_then(|x| Json::from_str(x).ok());
            //match jdata {
            //    Some(x) => {
            //        match scope.state.push_action(x) {
            //            Ok(id) => {
            //                respond(res, format, {
            //                    let mut h = HashMap::new();
            //                    h.insert("registered", id);
            //                    h
            //                })
            //            }
            //            Err(PushActionError::TooManyRequests) => {
            //                serve_error_page(429, res);
            //                Ok(())
            //            }
            //            Err(PushActionError::NotALeader) => {
            //                serve_error_page(421, res);
            //                Ok(())
            //            }
            //        }
            //    }
            //    None => {
            //        serve_error_page(400, res);
            //        Ok(())
            //    }
            //}
        }
        ActionIsPending(id) => {
            serve_error_page(NotImplemented)
            //respond(res, format, {
            //    let mut h = HashMap::new();
            //    h.insert("pending", scope.state.check_action(id));
            //    h
            //})
        }
    }
}
