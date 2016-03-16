use time::Timespec;

pub trait ToMsec {
    fn to_msec(&self) -> u64;
}

impl ToMsec for Timespec {
    fn to_msec(&self) -> u64 {
        return self.sec as u64 * 1000 + self.nsec as u64 / 1000000;
    }
}
