use std::time::Duration;

use abstract_ns::HostResolve;
use ns_router::{self, SubscribeExt, Router};
use ns_std_threaded;
use futures_cpupool::Builder;
use self_meter_http::Meter;
use tk_easyloop::handle;


pub fn init(meter: &Meter) -> Router {
    let ns_pool = {
        let m1 = meter.clone();
        let m2 = meter.clone();
        Builder::new()
        // TODO(tailhook) configure it
        .pool_size(2)
        .name_prefix("ns-resolver-")
        .after_start(move || m1.track_current_thread_by_name())
        .before_stop(move || m2.untrack_current_thread())
        .create()
    };
    ns_router::Router::from_config(&ns_router::Config::new()
        .set_fallthrough(ns_std_threaded::ThreadedResolver::use_pool(ns_pool)
            .null_service_resolver()
            .interval_subscriber(Duration::new(1, 0), &handle()))
        .done(),
        &handle())
}
