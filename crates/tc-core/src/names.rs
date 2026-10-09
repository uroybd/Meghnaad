//! Object names used by TaskChampion's cloud server. UUIDs appear in "simple" form
//! (lowercase hex, no hyphens).

use uuid::Uuid;

pub const LATEST: &str = "latest";
pub const SALT: &str = "salt";
pub const VERSION_PREFIX: &str = "v-";
pub const SNAPSHOT_PREFIX: &str = "s-";

pub fn version_name(parent: Uuid, child: Uuid) -> String {
    format!("v-{}-{}", parent.as_simple(), child.as_simple())
}

pub fn parse_version_name(name: &str) -> Option<(Uuid, Uuid)> {
    let rest = name.strip_prefix("v-")?;
    if !rest.is_ascii() || rest.len() != 32 + 1 + 32 || rest.as_bytes()[32] != b'-' {
        return None;
    }
    let parent = Uuid::try_parse(&rest[..32]).ok()?;
    let child = Uuid::try_parse(&rest[33..]).ok()?;
    Some((parent, child))
}

pub fn snapshot_name(version: Uuid) -> String {
    format!("s-{}", version.as_simple())
}

pub fn parse_snapshot_name(name: &str) -> Option<Uuid> {
    let rest = name.strip_prefix("s-")?;
    if !rest.is_ascii() || rest.len() != 32 {
        return None;
    }
    Uuid::try_parse(rest).ok()
}

/// Prefix matching all children of `parent`.
pub fn child_prefix(parent: Uuid) -> String {
    format!("v-{}-", parent.as_simple())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_name_round_trips() {
        let (p, c) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let name = version_name(p, c);
        assert_eq!(
            name,
            "v-00000000000000000000000000000001-00000000000000000000000000000002"
        );
        assert_eq!(parse_version_name(&name), Some((p, c)));
    }

    #[test]
    fn rejects_malformed_names() {
        assert_eq!(parse_version_name("v-abc"), None);
        assert_eq!(
            parse_version_name("x-00000000000000000000000000000001-00000000000000000000000000000002"),
            None
        );
        assert_eq!(parse_snapshot_name("s-zz"), None);
        assert_eq!(parse_snapshot_name("latest"), None);
    }

    #[test]
    fn snapshot_name_round_trips() {
        let v = Uuid::from_u128(0xdead_beef);
        assert_eq!(parse_snapshot_name(&snapshot_name(v)), Some(v));
    }
}
