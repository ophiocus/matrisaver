// Version primitives shared by the host crates' update checks.
// HTTP I/O is intentionally kept out of core — it only owns parsing and
// comparison. Host crates pull APP_VERSION from their own build.rs (set
// from the latest `git describe --tags` tag) and hit the GitHub Releases
// API directly. Compare against I:\Skeleton\src\git_update.rs.

/// The version baked into this *crate* at compile time. Hosts override
/// at their own layer with an APP_VERSION env var fed by build.rs and
/// derived from the latest `v*` git tag, so installed builds report the
/// release tag rather than the workspace version literal.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `owner/repo` pointing at the project's GitHub releases. The Windows
/// host derives the Releases-API URL from this:
///   https://api.github.com/repos/{APP_GH_REPO}/releases/latest
pub const APP_GH_REPO: &str = "ophiocus/matrisaver";

/// A parsed semantic version. 3-part canonical with a tolerated 4th
/// component (matches the I:\ family's `v0.1.0.003`-style tags). Build
/// numbers, when present, factor into ordering after patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer(pub u32, pub u32, pub u32, pub u32);

impl SemVer {
    /// Parse `"1.2.3"`, `"v1.2.3"`, or `"1.2.3.4"`. Pre-release suffixes
    /// (e.g. `-rc.1`) are stripped from the last numeric component.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim_start_matches('v');
        let s = s.split('-').next().unwrap_or(s);
        let mut parts = s.split('.');
        let major: u32 = parts.next()?.parse().ok()?;
        let minor: u32 = parts.next().unwrap_or("0").parse().ok()?;
        let patch: u32 = parts.next().unwrap_or("0").parse().ok()?;
        let build: u32 = parts.next().unwrap_or("0").parse().ok()?;
        Some(Self(major, minor, patch, build))
    }

    /// Returns `true` if `self` is strictly newer than `other`.
    pub fn is_newer_than(self, other: Self) -> bool {
        self > other
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.3 == 0 {
            write!(f, "{}.{}.{}", self.0, self.1, self.2)
        } else {
            write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        assert_eq!(SemVer::parse("1.2.3"), Some(SemVer(1, 2, 3, 0)));
        assert_eq!(SemVer::parse("v0.1.0"), Some(SemVer(0, 1, 0, 0)));
    }

    #[test]
    fn parse_four_part() {
        assert_eq!(SemVer::parse("0.1.0.3"), Some(SemVer(0, 1, 0, 3)));
        assert_eq!(SemVer::parse("v1.2.3.45"), Some(SemVer(1, 2, 3, 45)));
    }

    #[test]
    fn parse_pre_release_stripped() {
        assert_eq!(SemVer::parse("1.2.3-rc.1"), Some(SemVer(1, 2, 3, 0)));
        assert_eq!(SemVer::parse("1.2.3.4-beta"), Some(SemVer(1, 2, 3, 4)));
    }

    #[test]
    fn ordering() {
        let old = SemVer::parse("0.1.0").unwrap();
        let new = SemVer::parse("0.2.0").unwrap();
        assert!(new.is_newer_than(old));
        assert!(!old.is_newer_than(new));
        assert!(!old.is_newer_than(old));
    }

    #[test]
    fn build_orders_after_patch() {
        let v3 = SemVer::parse("0.1.0").unwrap();
        let v3_b1 = SemVer::parse("0.1.0.1").unwrap();
        assert!(v3_b1.is_newer_than(v3));
    }

    #[test]
    fn display_drops_trailing_zero_build() {
        assert_eq!(SemVer(1, 2, 3, 0).to_string(), "1.2.3");
        assert_eq!(SemVer(1, 2, 3, 4).to_string(), "1.2.3.4");
    }
}
