use std::path::Path;

pub fn relative<'x>(path: &'x Path, relative_to: &'x Path) -> Option<&'x Path> {
    let mut iter = path.components();
    for (their, my) in relative_to.components().zip(iter.by_ref()) {
        if my != their {
            return None;
        }
    }
    Some(iter.as_path())
}
