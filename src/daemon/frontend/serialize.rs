use std::time::{SystemTime, Duration, UNIX_EPOCH};


pub fn ms_to_system_time(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}
