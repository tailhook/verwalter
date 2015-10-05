use time::{SteadyTime, Duration};



struct TimeScale {
    now: SteadyTime,
}

impl TimeScale {
    fn advance_ms(&mut self, ms: u64) {
        self.now += Duration::milliseconds(ms);
    }
}
