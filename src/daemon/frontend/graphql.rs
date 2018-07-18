use std::sync::{Arc};
use std::time::SystemTime;
use std::collections::HashMap;

use juniper::{self, InputValue, RootNode, FieldError, execute};
use juniper::{Value, ExecutionError};
use serde_json::{Value as Json, to_value};
use tk_http::Status;
use tk_http::server::{Error};

use time_util::ToMsec;
use frontend::{Request};
use frontend::{Config, read_json};
use frontend::routing::Format;
use frontend::error_page::{error_page};
use frontend::api::{respond};
use frontend::status;
use shared::SharedState;


pub struct ContextRef<'a> {
    pub state: &'a SharedState,
    pub config: &'a Config,
}

#[derive(Clone)]
pub struct Context {
    pub state: SharedState,
    pub config: Arc<Config>,
}

pub type Schema<'a> = RootNode<'a, &'a Query, &'a Mutation>;

pub struct Query;
pub struct Mutation;
pub struct Timestamp(pub SystemTime);

#[derive(Deserialize, Clone, Debug)]
pub struct Input {
    pub query: String,
    #[serde(default, rename="operationName")]
    pub operation_name: Option<String>,
    #[serde(default)]
    pub variables: Option<HashMap<String, InputValue>>,
}

#[derive(Debug, Serialize)]
pub struct Output {
    #[serde(skip_serializing_if="Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if="ErrorWrapper::is_empty")]
    pub errors: ErrorWrapper,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ErrorWrapper {
    Execution(Vec<ExecutionError>),
    Fatal(Json),
}

#[derive(Debug, Serialize, GraphQLObject)]
pub struct Okay {
    ok: bool,
}

graphql_object!(<'a> &'a Query: ContextRef<'a> as "Query" |&self| {
    field status(&executor) -> Result<status::GData, FieldError> {
        status::graph(executor.context())
    }
});

graphql_object!(<'a> &'a Mutation: ContextRef<'a> as "Mutation" |&self| {
    field noop(&executor) -> Result<Okay, FieldError> {
        Ok(Okay { ok: true })
    }
});

graphql_scalar!(Timestamp {
    description: "A timestamp transferred as a number of milliseconds"

    resolve(&self) -> Value {
        Value::float(self.0.to_msec() as f64)
    }

    from_input_value(v: &InputValue) -> Option<Timestamp> {
        unimplemented!();
    }
});

pub fn serve<S: 'static>(state: &SharedState, config: &Arc<Config>, format: Format)
    -> Result<Request<S>, Error>
{
    let state = state.clone();
    let config = config.clone();
    Ok(read_json(move |input: Input, e| {
        let context = ContextRef { state: &state, config: &*config };

        let variables = input.variables.unwrap_or_else(HashMap::new);

        let result = execute(&input.query,
            input.operation_name.as_ref().map(|x| &x[..]),
            &Schema::new(&Query, &Mutation),
            &variables,
            &context);
        let out = match result {
            Ok((data, errors)) => {
                Output {
                    data: Some(data),
                    errors: ErrorWrapper::Execution(errors),
                }
            }
            Err(e) => {
                Output {
                    data: None,
                    errors: ErrorWrapper::Fatal(
                        to_value(&e).expect("can serialize error")),
                }
            }
        };

        if out.data.is_some() {
            Box::new(respond(e, format, out))
        } else {
            Box::new(error_page(Status::BadRequest, e))
        }
    }))
}

pub fn ws_response<'a>(context: &Context, input: &'a Input) -> Output {
    let context = ContextRef {
        state: &context.state,
        config: &*context.config,
    };

    let empty = HashMap::new();
    let variables = input.variables.as_ref().unwrap_or(&empty);

    let result = execute(&input.query,
        input.operation_name.as_ref().map(|x| &x[..]),
        &Schema::new(&Query, &Mutation),
        &variables,
        &context);

    match result {
        Ok((data, errors)) => {
            Output {
                data: Some(data),
                errors: ErrorWrapper::Execution(errors),
            }
        }
        Err(e) => {
            Output {
                data: None,
                errors: ErrorWrapper::Fatal(
                    to_value(&e).expect("can serialize error")),
            }
        }
    }
}

impl ErrorWrapper {
    fn is_empty(&self) -> bool {
        use self::ErrorWrapper::*;
        match self {
            Execution(v) => v.is_empty(),
            Fatal(..) => false,
        }
    }
}

impl<'a> juniper::Context for ContextRef<'a> {}
