use std::ascii::AsciiExt;
use std::str::from_utf8;
use std::path::Path;
use std::path::Component::Normal;

use tk_http::server::Head;


#[derive(Clone, Debug)]
pub enum ApiRoute {
    Status,
    Peers,
    Schedule,
    SchedulerInput,
    SchedulerDebugInfo,
    Election,
    PushAction,
    ActionIsPending(u64),
    PendingActions,
    ForceRenderAll,
}

#[derive(Clone, Copy, Debug)]
pub enum Range {
    FromTo(u64, u64),
    AllFrom(u64),
    Last(u64),
}


#[derive(Clone, Debug)]
pub enum LogRoute {
    Index(String, Range),
    Global(String, Range),
    Changes(String, Range),
    Role(String, Range),
    External(String, Range),
}

#[derive(Clone, Debug, Copy)]
pub enum Format {
    Json,
    Gron,
    Plain,
}

#[derive(Clone, Debug)]
pub enum Route {
    Index,
    Static(String),
    Api(ApiRoute, Format),
    Log(LogRoute),
    NotFound,
}

fn path_component(path: &str) -> (&str, &str) {
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

fn parse_range_bytes(x: &str) -> Option<Range> {
    use self::Range::*;
    let mut iter = x.splitn(2, "-");
    match (iter.next(), iter.next()) {
        (_, None) | (None, _) => return None,
        (Some(""), Some(neg)) => neg.parse().ok().map(Last),
        (Some(st), Some("")) => st.parse().ok().map(AllFrom),
        (Some(start), Some(end)) => {
            start.parse().and_then(
                |start| end.parse().map(|end| FromTo(start, end))
            ).ok()
        }
    }
}

fn parse_range(head: &Head) -> Option<(&'static str, Range)> {
    let mut result = None;
    for (name, value) in head.headers() {
        if name.eq_ignore_ascii_case("Range") {
            let s = match from_utf8(value) {
                Ok(s) => s,
                // TODO(tailhook) implement 416 or 400
                Err(..) => return None,
            };
            if result.is_some() {
                // TODO(tailhook) implement 416 or 400
                return None;
            }
            if s.trim().starts_with("bytes=") {
                match parse_range_bytes(&s[6..]) {
                    Some(x) => result = Some(("bytes", x)),
                    // TODO(tailhook) implement 400
                    None => return None,
                }
            } else if s.trim().starts_with("records=") {
                match parse_range_bytes(&s[8..]) {
                    Some(x) => result = Some(("records", x)),
                    // TODO(tailhook) implement 400
                    None => return None,
                }
            } else {
                // TODO(tailhook) implement 400
                return None;
            }
        }
    }
    return result;
}

fn validate_path<P: AsRef<Path>>(path: P) -> bool {
    for cmp in Path::new(path.as_ref()).components(){
        match cmp {
            Normal(_) => {}
            _ => return false,
        }
    }
    return true;
}

fn parse_log_route(path: &str, head: &Head) -> Option<LogRoute> {
    use self::LogRoute::*;
    if !validate_path(path) {
        // TODO(tailhook) implement 400
        return None;
    }
    // TODO(tailhook) implement 416
    parse_range(head).and_then(|(typ, rng)| {
        match (path_component(path), typ) {
            (("index", tail), "bytes") => Some(Index(tail.into(), rng)),
            (("global", tail), "bytes") => Some(Global(tail.into(), rng)),
            (("changes", tail), "bytes") => Some(Changes(tail.into(), rng)),
            (("role", tail), "bytes") => Some(Role(tail.into(), rng)),
            (("external", tail), "bytes") => Some(External(tail.into(), rng)),
            _ => None,
        }
    })
}

fn parse_api(path: &str, head: &Head) -> Option<Route> {
    use self::Route::*;
    use self::ApiRoute::*;
    use self::Format::Plain;
    match path_component(path) {
        ("status", "") => Some(Api(Status, api_suffix(path))),
        ("peers", "") => Some(Api(Peers, api_suffix(path))),
        ("schedule", "") => Some(Api(Schedule, api_suffix(path))),
        ("scheduler_input", "") => Some(Api(SchedulerInput, api_suffix(path))),
        ("scheduler_debug_info", "") => Some(Api(SchedulerDebugInfo, Plain)),
        ("election", "") => Some(Api(Election, api_suffix(path))),
        ("action", "") => Some(Api(PushAction, api_suffix(path))),
        ("force_render_all", "") => Some(Api(ForceRenderAll, Plain)),
        ("action_is_pending", tail) => {
            tail.parse().map(|x| {
                Api(ActionIsPending(x), api_suffix(path))
            }).ok()
        }
        ("pending_actions", "") => Some(Api(PendingActions, api_suffix(path))),
        ("log", tail) => parse_log_route(tail, &head).map(Log),
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
        ("", _) => Some(Index),
        ("v1", suffix) => parse_api(suffix, &head),
        ("common", path) => {
            if !validate_path(path) {
                // TODO(tailhook) implement 400
                return Route::NotFound;
            }
            Some(Static(path.to_string()))
        },
        (_, _) => Some(Index),
    };
    debug!("Routed {:?} to {:?}", path, route);
    route.unwrap_or(Route::NotFound)
}
