use sha1::Sha1;

pub fn hash<S: AsRef<[u8]>>(obj: S) -> String {
    let mut sha = Sha1::new();
    sha.update(obj.as_ref());
    return sha.digest().to_string();
}
