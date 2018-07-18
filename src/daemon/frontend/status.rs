use std::i32;
use std::sync::Arc;

use juniper::{FieldError};
use self_meter_http::Meter;
use serde_json;

use id::Id;
use peer;
use elect::ElectionState;
use frontend::graphql::Timestamp;
use frontend::graphql::ContextRef;


pub struct GData<'a> {
    ctx: &'a ContextRef<'a>,
}

pub struct GProcessReport(Meter);
pub struct GThreadsReport(Meter);
pub struct Peers(Arc<peer::Peers>);
pub struct Election(Arc<ElectionState>);

graphql_object!(<'a> GData<'a>: () as "Status" |&self| {
    description: "Status data for the verwalter itself"
    field version() -> &'static str {
        env!("CARGO_PKG_VERSION")
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
    field roles() -> i32 {
        self.ctx.state.num_roles() as i32
    }
    field peers() -> Peers {
        Peers(self.ctx.state.peers())
    }
    field election() -> Election {
        Election(self.ctx.state.election())
    }
    field num_errors() -> i32 {
        (self.ctx.state.errors().len() + self.ctx.state.failed_roles().len())
        as i32
    }
    field debug_force_leader() -> bool {
        self.ctx.state.debug_force_leader()
    }
    field self_report() -> GProcessReport {
        GProcessReport(self.ctx.state.meter.clone())
    }
    field threads_report() -> GThreadsReport {
        GThreadsReport(self.ctx.state.meter.clone())
    }
});

graphql_object!(Peers: () as "Peers" |&self| {
    field number() -> i32 {
        self.0.peers.len() as i32
    }
    field timestamp() -> Timestamp {
        Timestamp(self.0.timestamp)
    }
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

pub fn graph<'a>(ctx: &'a ContextRef<'a>) -> Result<GData<'a>, FieldError>
{
    Ok(GData { ctx })
}
