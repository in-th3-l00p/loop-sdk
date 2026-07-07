mod runner;

pub(crate) use runner::apply;

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: String,
    pub sql: String,
}

impl Migration {
    pub fn new(version: i64, name: impl Into<String>, sql: impl Into<String>) -> Self {
        Migration {
            version,
            name: name.into(),
            sql: sql.into(),
        }
    }
}

pub fn checksum(sql: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in sql.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_are_stable_and_content_sensitive() {
        assert_eq!(
            checksum("CREATE TABLE t (id BIGINT)"),
            checksum("CREATE TABLE t (id BIGINT)")
        );
        assert_ne!(
            checksum("CREATE TABLE t (id BIGINT)"),
            checksum("CREATE TABLE t (id TEXT)")
        );
        assert_eq!(checksum(""), "cbf29ce484222325");
    }
}
