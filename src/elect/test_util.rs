use time::{SteadyTime, Duration};



pub struct TimeScale {
    now: SteadyTime,
}

impl TimeScale {
    pub fn new() -> TimeScale {
        TimeScale {
            // unfortunately we can't create arbitrary steady time value
            now: SteadyTime::now(),
        }
    }
    pub fn advance_ms(&mut self, ms: i64) {
        self.now = self.now +  Duration::milliseconds(ms);
    }
    pub fn now(&self) -> SteadyTime {
        self.now
    }
}
