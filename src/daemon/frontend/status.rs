use std::i32;

use juniper::{FieldError};
use self_meter_http::Meter;
use serde_json;

use frontend::graphql::ContextRef;


pub struct GData<'a> {
    ctx: &'a ContextRef<'a>,
}

pub struct GProcessReport(Meter);
pub struct GThreadsReport(Meter);

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
