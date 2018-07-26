use std::i32;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use juniper::{FieldError};
use self_meter_http::Meter;
use serde_json;

use id::Id;
use peer;
use elect::ElectionState;
use scheduler;
use fetch;
use frontend::graphql::Timestamp;
use frontend::graphql::ContextRef;


pub struct GData<'a> {
    ctx: &'a ContextRef<'a>,
}

struct LeaderInfo {
    id: Id,
    name: String,
    hostname: String,
    addr: Option<String>,
    schedule: Option<String>,
    debug_forced: bool,
}

pub struct GProcessReport(Meter);
pub struct GThreadsReport(Meter);
pub struct Peers(Arc<peer::Peers>);
pub struct Peer(Arc<peer::Peer>);
pub struct Election(Arc<ElectionState>);
pub struct Schedule(Arc<scheduler::Schedule>);
pub struct ScheduleData(Arc<scheduler::Schedule>);
pub struct FetchState(Arc<fetch::PublicState>);
pub struct Roles<'a>(&'a ContextRef<'a>);

graphql_object!(<'a> GData<'a>: () as "Status" |&self| {
    description: "Status data for the verwalter itself"
    field version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    field id() -> &Id {
        &self.ctx.state.id
    }
    field name() -> &str {
        &self.ctx.state.name
    }
    field hostname() -> &str {
        &self.ctx.state.hostname
    }
    field default_frontend() -> &str {
        &self.ctx.config.default_frontend
    }
    field roles() -> Roles {
        Roles(self.ctx)
    }
    field peers() -> Peers {
        Peers(self.ctx.state.peers())
    }
    field election() -> Election {
        Election(self.ctx.state.election())
    }
    field schedule() -> Option<Schedule> {
        self.ctx.state.schedule().map(Schedule)
    }
    field fetch() -> FetchState {
        FetchState(self.ctx.state.fetch_state.get())
    }
    field num_errors() -> i32 {
        (self.ctx.state.errors().len() + self.ctx.state.failed_roles().len())
        as i32
    }
    field leader() -> Option<LeaderInfo> {
        let election = self.ctx.state.election();
        if election.is_leader {
            let owned_schedule = self.ctx.state.owned_schedule();
            Some(LeaderInfo {
                id: self.ctx.state.id().clone(),
                name: self.ctx.state.name.clone(),
                hostname: self.ctx.state.hostname.clone(),
                // TODO(tailhook) resolve listening address and show
                addr: None,
                schedule: owned_schedule.as_ref().map(|x| x.hash.clone()),
                debug_forced: self.ctx.state.debug_force_leader(),
            })
        } else {
            let peers = self.ctx.state.peers();
            match election.leader.as_ref()
                .and_then(|id| peers.peers.get(id).map(|p| (id, p)))
            {
                Some((id, peer)) => {
                    let leader_peer = peer.get();
                    let schedule_hash = leader_peer.schedule.as_ref()
                                        .map(|x| &x.hash);
                    Some(LeaderInfo {
                        id: id.clone(),
                        name: leader_peer.name.clone(),
                        hostname: leader_peer.hostname.clone(),
                        addr: leader_peer.addr
                            .map(|x| x.to_string()),
                        schedule: schedule_hash.cloned(),
                        debug_forced: false,
                    })
                }
                None => None,
            }
        }
    }
    field self_report() -> GProcessReport {
        GProcessReport(self.ctx.state.meter.clone())
    }
    field threads_report() -> GThreadsReport {
        GThreadsReport(self.ctx.state.meter.clone())
    }
    field schedule_status() -> &'static str {
        let election = self.ctx.state.election();
        if election.is_leader {
            "ok"
        } else {
            let peers = self.ctx.state.peers();
            let stable_schedule = self.ctx.state.schedule();
            match election.leader.as_ref()
                .and_then(|id| peers.peers.get(id).map(|p| (id, p)))
            {
                Some((id, peer)) => {
                    let leader_peer = peer.get();
                    let schedule_hash = leader_peer.schedule.as_ref()
                                        .map(|x| &x.hash);
                    match (schedule_hash, &stable_schedule) {
                        (Some(h), &Some(ref s)) if h == &s.hash => "ok",
                        (Some(_), _) => "fetching",
                        (None, _) => "waiting",
                    }
                }
                None => "unstable",
            }
        }
    }
});

graphql_object!(Peers: () as "Peers" |&self| {
    field number() -> i32 {
        self.0.peers.len() as i32
    }
    field errorneous() -> Vec<Peer> {
        let mut arr = self.0.peers.values()
            .map(|p| p.get())
            .filter(|p| p.errors > 0)
            .map(Peer).collect::<Vec<_>>();
        arr.sort_unstable_by(|a, b| a.0.name.cmp(&b.0.name));
        return arr;
    }
    field timestamp() -> Timestamp {
        Timestamp(self.0.timestamp)
    }
});

graphql_object!(Peer: () as "Peer" |&self| {
    field id() -> &Id { &self.0.id }
    field name() -> &String { &self.0.name }
    field hostname() -> &String { &self.0.hostname }
});

graphql_object!(<'a> Roles<'a>: () as "Roles" |&self| {
    field number() -> i32 {
        self.0.state.num_roles() as i32
    }
    // TODO(tailhook) remove clone once juniper supports it
    field failed() -> Vec<String> {
        let mut vec = self.0.state.failed_roles().iter().cloned()
            .collect::<Vec<_>>();
        vec.sort_unstable();
        return vec;
    }
});

graphql_object!(LeaderInfo: () as "Leader" |&self| {
    field id() -> &Id { &self.id }
    field name() -> &String { &self.name }
    field hostname() -> &String { &self.hostname }
    field addr() -> &Option<String> { &self.addr }
    field schedule() -> &Option<String> { &self.schedule }
    field debug_forced() -> bool { self.debug_forced }
});

graphql_object!(Election: () as "Election" |&self| {
    field is_leader() -> bool {
        self.0.is_leader
    }
    field is_stable() -> bool {
        self.0.is_stable
    }
    field promoting() -> &Option<Id> {
        &self.0.promoting
    }
    field num_votes_for_me() -> Option<i32> {
        self.0.num_votes_for_me.map(|x| x as i32)
    }
    field needed_votes() -> Option<i32> {
        self.0.num_votes_for_me.map(|x| x as i32)
    }
    field epoch() -> f64 {
        self.0.epoch as f64
    }
    field deadline() -> Timestamp {
        Timestamp(self.0.deadline)
    }
    field last_stable_timestamp() -> Option<Timestamp> {
        self.0.last_stable_timestamp.map(Timestamp)
    }
});

graphql_object!(Schedule: () as "Schedule" |&self| {
    field timestamp() -> Timestamp {
        Timestamp(UNIX_EPOCH + Duration::from_millis(self.0.timestamp))
    }
    field hash() -> &String {
        &self.0.hash
    }
    field data() -> ScheduleData {
        ScheduleData(self.0.clone())
    }
    field origin() -> &Id {
        &self.0.origin
    }
});

graphql_object!(FetchState: () as "Fetch" |&self| {
    field state() -> fetch::GraphqlState {
        self.0.to_graphql_state()
    }
});

// TODO(tailhook) rather make a serializer
fn convert(val: serde_json::Value) -> ::juniper::Value {
    use serde_json::Value as I;
    use juniper::Value as O;
    match val {
        I::Null => O::Null,
        I::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i <= i32::MAX as i64 && i >= i32::MIN as i64 {
                    O::Int(i as i32)
                } else {
                    O::Float(i as f64)
                }
            } else {
                O::Float(n.as_f64().expect("can alwasy be float"))
            }
        }
        I::String(s) => O::String(s),
        I::Bool(v) => O::Boolean(v),
        I::Array(items) => O::List(items.into_iter().map(convert).collect()),
        I::Object(map) => {
            O::Object(map.into_iter().map(|(k, v)| (k, convert(v))).collect())
        }
    }
}

fn convert_ref(val: &serde_json::Value) -> ::juniper::Value {
    use serde_json::Value as I;
    use juniper::Value as O;
    match val {
        I::Null => O::Null,
        I::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i <= i32::MAX as i64 && i >= i32::MIN as i64 {
                    O::Int(i as i32)
                } else {
                    O::Float(i as f64)
                }
            } else {
                O::Float(n.as_f64().expect("can alwasy be float"))
            }
        }
        I::String(s) => O::String(s.clone()),
        I::Bool(v) => O::Boolean(*v),
        I::Array(items) => O::List(items.iter().map(convert_ref).collect()),
        I::Object(map) => {
            O::Object(map.iter().map(|(k, v)| (k.clone(), convert_ref(v))).collect())
        }
    }
}

graphql_scalar!(GProcessReport as "ProcessReport" {
    description: "process perfromance information"
    resolve(&self) -> Value {
        convert(serde_json::to_value(self.0.process_report())
            .expect("serialize ProcessReport"))
    }
    from_input_value(_val: &InputValue) -> Option<GProcessReport> {
        unimplemented!();
    }
});

graphql_scalar!(GThreadsReport as "ThreadsReport" {
    description: "per-thread performance information"
    resolve(&self) -> Value {
        convert(serde_json::to_value(self.0.thread_report())
            .expect("serialize ThreadReport"))
    }
    from_input_value(_val: &InputValue) -> Option<GThreadsReport> {
        unimplemented!();
    }
});

graphql_scalar!(ScheduleData as "ScheduleData" {
    description: "dump of schedule data"
    resolve(&self) -> Value {
        convert_ref(&self.0.data)
    }
    from_input_value(_val: &InputValue) -> Option<ScheduleData> {
        unimplemented!();
    }
});

pub fn graph<'a>(ctx: &'a ContextRef<'a>) -> Result<GData<'a>, FieldError>
{
    Ok(GData { ctx })
}
