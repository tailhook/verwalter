use abstract_ns::{Router, RouterBuilder};
use ns_std_threaded::ThreadedResolver;
use futures_cpupool::Builder;
use self_meter_http::Meter;

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
    let ns = ThreadedResolver::new(ns_pool);
    let mut rb = RouterBuilder::new();
    rb.add_default(ns);
    rb.into_resolver()
}
