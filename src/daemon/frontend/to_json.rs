use libcantal;
use rustc_serialize::json::Json;


/// This is a trait similar to `rust_serialize::json::ToJson` but allows to
/// implement or own conversions (because of orphan rules)
pub trait ToJson {
    fn js(&self) -> Json;
}

impl ToJson for libcantal::Counter {
    fn js(&self) -> Json {
        Json::U64(self.get() as u64)
    }
}

impl ToJson for libcantal::Integer {
    fn js(&self) -> Json {
        Json::I64(self.get() as i64)
    }
}

impl<T: ToJson> ToJson for AsRef<T> {
    fn js(&self) -> Json {
        self.as_ref().js()
    }
}
