use time::{Timespec, SteadyTime, get_time};
use std::time::{SystemTime, UNIX_EPOCH, Duration};

pub trait ToMsec {
    fn to_msec(&self) -> u64;
}

impl ToMsec for Timespec {
    fn to_msec(&self) -> u64 {
        return self.sec as u64 * 1000 + self.nsec as u64 / 1000000;
    }
}
impl ToMsec for SteadyTime {
    fn to_msec(&self) -> u64 {
        (get_time() + (*self - SteadyTime::now())).to_msec()
    }
}

impl ToMsec for SystemTime {
    fn to_msec(&self) -> u64 {
        (self.duration_since(UNIX_EPOCH).unwrap()).to_msec()
    }
}

impl ToMsec for Duration {
    fn to_msec(&self) -> u64 {
        self.as_secs() * 1000 + (self.subsec_nanos() / 1000_000) as u64
    }
}
