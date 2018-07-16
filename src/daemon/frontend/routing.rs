use std::path::Path;
use std::path::Component::Normal;

use tk_http::server::Head;

#[derive(Clone, Debug)]
pub struct Query {
    pub path: String,
}

#[derive(Clone, Debug)]
pub enum ApiRoute {
    Graphql,
    Graphiql,
    Status,
    Peers,
    Schedule,
    SchedulerInput,
    SchedulerDebugInfo,
    Election,
    Backups,
    Backup(String),
    PushAction,
    WaitAction,
    ActionIsPending(u64),
    PendingActions,
    RolesData,
    Query(Query),
    ForceRenderAll,
    RedirectByNodeName,
}

#[derive(Clone, Debug)]
pub enum LogRoute {
    Index(String),
    Global(String),
    Changes(String),
    Role(String),
    External(String),
}

#[derive(Clone, Debug, Copy)]
pub enum Format {
    Json,
    Gron,
    Plain,
}

#[derive(Clone, Debug)]
pub enum Route {
    CommonIndex,
    CommonStatic(String),
    AlterIndex(String),
    AlterStatic(String),
    Api(ApiRoute, Format),
    Log(LogRoute),
    WasmScheduler,
    WasmQuery,
    NotFound,
    BadContentType,
}

pub fn path_component(path: &str) -> (&str, &str) {
    let path = if path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    match path.bytes().position(|x| x == b'/') {
        Some(end) => (&path[..end], &path[end+1..]),
        None => {
            let end = path.bytes().position(|x| x == b'.')
                .unwrap_or(path.as_bytes().len());
            (&path[..end], "")
        }
    }
}

fn suffix(path: &str) -> &str {
    match path.bytes().rposition(|x| x == b'.' || x == b'/') {
        Some(i) if path.as_bytes()[i] == b'.' => &path[i+1..],
        Some(_) => "",
        None => "",
    }
}

fn api_suffix(path: &str) -> Format {
    use self::Format::*;
    match suffix(path) {
        "pretty" => Plain,
        "gron" => Gron,
        _ => Json,
    }
}

fn validate_path<P: AsRef<Path>>(path: P) -> bool {
    for cmp in Path::new(path.as_ref()).components() {
        match cmp {
            Normal(_) => {}
            _ => return false,
        }
    }
    return true;
}

fn parse_log_route(path: &str) -> Option<LogRoute> {
    use self::LogRoute::*;
    if !validate_path(path) {
        // TODO(tailhook) implement 400
        return None;
    }
    match path_component(path) {
        ("index", tail) => Some(Index(tail.into())),
        ("global", tail) => Some(Global(tail.into())),
        ("changes", tail) => Some(Changes(tail.into())),
        ("role", tail) => Some(Role(tail.into())),
        ("external", tail) => Some(External(tail.into())),
        _ => None,
    }
}

fn parse_api(path: &str, content_type: Option<&[u8]>) -> Option<Route> {
    use self::Route::*;
    use self::ApiRoute::*;
    use self::Format::Plain;
    match path_component(path) {
        ("status", "") => Some(Api(Status, api_suffix(path))),
        ("graphql", "") => Some(Api(Graphql, api_suffix(path))),
        ("graphiql", "") => Some(Api(Graphiql, api_suffix(path))),
        ("leader-redirect-by-node-name", "") => {
            Some(Api(RedirectByNodeName, Plain))
        }
        ("peers", "") => Some(Api(Peers, api_suffix(path))),
        ("schedule", "") => Some(Api(Schedule, api_suffix(path))),
        ("scheduler_input", "") => Some(Api(SchedulerInput, api_suffix(path))),
        ("scheduler_debug_info", "") => Some(Api(SchedulerDebugInfo, Plain)),
        ("election", "") => Some(Api(Election, api_suffix(path))),
        ("backups", "") => Some(Api(Backups, api_suffix(path))),
        ("backup", name) => {
            if name.chars().all(|x| x.is_alphanumeric() || x == '-') {
                Some(Api(Backup(name.to_string()), api_suffix(path)))
            } else {
                None
            }
        }

        ("action", "") if content_type == Some(b"application/json")
            => Some(Api(PushAction, api_suffix(path))),
        ("action", "") => Some(Route::BadContentType),

        ("wait_action", "") if content_type == Some(b"application/json")
            => Some(Api(WaitAction, api_suffix(path))),
        ("wait_action", "") => Some(Route::BadContentType),

        ("force_render_all", "") => Some(Api(ForceRenderAll, Plain)),
        ("action_is_pending", tail) => {
            tail.parse().map(|x| {
                Api(ActionIsPending(x), api_suffix(path))
            }).ok()
        }
        ("pending_actions", "") => Some(Api(PendingActions, api_suffix(path))),
        ("roles_data", "") => Some(Api(RolesData, api_suffix(path))),
        ("query", tail) if content_type == Some(b"application/json")
            => {
                Some(Api(Query(self::Query {
                    path: tail.to_string(),
                }), api_suffix(path)))
            }
        ("query", _) => Some(Route::BadContentType),
        ("log", tail) => parse_log_route(tail).map(Log),
        ("wasm", "scheduler.wasm") => Some(Route::WasmScheduler),
        ("wasm", "query.wasm") => Some(Route::WasmQuery),
        _ => None,
    }
}

pub fn route(head: &Head) -> Route {
    use self::Route::*;
    let path = if let Some(path) = head.path() {
        path
    } else {
        return Route::NotFound;
    };
    let path = match path.find('?') {
        Some(x) => &path[..x],
        None => path,
    };
    let route = match path_component(&path[..]) {
        ("", _) => Some(CommonIndex),
        ("v1", suffix) => {
            let mut content_type = None;
            for (name, value) in head.headers() {
                if name.eq_ignore_ascii_case("Content-Type") {
                    content_type = Some(value);
                    break;
                }
            }
            parse_api(suffix, content_type)
        }
        (dir, suffix) if dir.starts_with("~") => {
            if !validate_path(&path[2..]) {
                // TODO(tailhook) implement 400
                return Route::NotFound;
            }
            match path_component(suffix) {
                ("js", _) | ("css", _) | ("fonts", _) | ("img", _) |
                ("files", _) => {
                    Some(AlterStatic(path[2..].to_string()))
                }
                _ => {
                    Some(AlterIndex(dir[1..].to_string()))
                }
            }
        }
        // this is kinda legacy for now
        ("common", path) => {
            if !validate_path(path) {
                // TODO(tailhook) implement 400
                return Route::NotFound;
            }
            Some(CommonStatic(path.to_string()))
        },
        (_, _) => Some(CommonIndex),
    };
    debug!("Routed {:?} to {:?}", path, route);
    route.unwrap_or(Route::NotFound)
}
