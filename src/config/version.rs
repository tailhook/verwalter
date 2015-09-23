
/// Version is just bare string wrapped into a struct that can be compared
/// in a smart way
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Version(String);


// TODO(tailhook)
// impl PartialCmp for Version {
// }
