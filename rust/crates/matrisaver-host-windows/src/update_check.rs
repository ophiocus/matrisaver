// GitHub Releases-driven update check for the Windows host /c (config)
// mode. Modeled on I:\Skeleton\src\git_update.rs; deliberately uses
// reqwest + native-tls so we hit Windows SChannel and stay off the
// rustls/ring path.
//
// Not called during screensaver or preview execution — only when the
// user opens the screensaver settings dialog from Display Properties.

use matrisaver_core::update::{SemVer, APP_GH_REPO};
use serde::Deserialize;

/// One asset entry in the GitHub Releases JSON response.
#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
}

/// Subset of the GitHub Releases API payload we care about.
#[derive(Debug, Deserialize)]
struct ReleasePayload {
    tag_name: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

/// Result returned to the caller. All variants are non-fatal — the
/// settings dialog should never refuse to open because the network is
/// down.
#[derive(Debug)]
pub enum UpdateCheckResult {
    UpToDate {
        current: String,
    },
    Available {
        current: String,
        latest: String,
        msi_url: String,
        changelog_url: Option<String>,
    },
    Failed(String),
}

/// Build the API URL. `repo_override` takes precedence over the
/// MATRISAVER_GH_REPO env var, then falls back to APP_GH_REPO.
fn api_url(repo_override: Option<&str>) -> String {
    let repo = repo_override
        .map(str::to_owned)
        .or_else(|| std::env::var("MATRISAVER_GH_REPO").ok())
        .unwrap_or_else(|| APP_GH_REPO.to_owned());
    format!("https://api.github.com/repos/{repo}/releases/latest")
}

/// Perform a synchronous update check.
///
/// `repo_override` lets `--update-check-repo owner/repo` flip the target
/// without rebuilding (useful for staging channels or CI tests).
pub fn check(repo_override: Option<&str>) -> UpdateCheckResult {
    let url = api_url(repo_override);
    let user_agent = format!("MatriSaver/{}", env!("APP_VERSION"));

    let client = match reqwest::blocking::Client::builder()
        .user_agent(user_agent)
        .timeout(std::time::Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(err) => return UpdateCheckResult::Failed(format!("client build: {err}")),
    };

    let response = match client.get(&url).send() {
        Ok(r) => r,
        Err(err) => return UpdateCheckResult::Failed(err.to_string()),
    };

    if !response.status().is_success() {
        return UpdateCheckResult::Failed(format!("HTTP {}", response.status().as_u16()));
    }

    let payload: ReleasePayload = match response.json() {
        Ok(p) => p,
        Err(err) => return UpdateCheckResult::Failed(format!("payload parse: {err}")),
    };

    let current_str = env!("APP_VERSION");
    let current_ver = match SemVer::parse(current_str) {
        Some(v) => v,
        None => {
            return UpdateCheckResult::Failed(format!(
                "could not parse compiled APP_VERSION '{current_str}'"
            ))
        }
    };

    let latest_ver = match SemVer::parse(&payload.tag_name) {
        Some(v) => v,
        None => {
            return UpdateCheckResult::Failed(format!(
                "could not parse remote tag '{}'",
                payload.tag_name
            ))
        }
    };

    if !latest_ver.is_newer_than(current_ver) {
        return UpdateCheckResult::UpToDate {
            current: current_str.to_owned(),
        };
    }

    let msi_url = match payload
        .assets
        .iter()
        .find(|a| a.name.to_ascii_lowercase().ends_with(".msi"))
    {
        Some(asset) => asset.browser_download_url.clone(),
        None => {
            return UpdateCheckResult::Failed(format!(
                "no .msi asset on release {}",
                payload.tag_name
            ))
        }
    };

    UpdateCheckResult::Available {
        current: current_str.to_owned(),
        latest: payload.tag_name.trim_start_matches('v').to_owned(),
        msi_url,
        changelog_url: payload.html_url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_url_uses_default_repo() {
        let url = api_url(None);
        assert!(url.starts_with("https://api.github.com/repos/"));
        assert!(url.ends_with("/releases/latest"));
    }

    #[test]
    fn api_url_honors_override() {
        let url = api_url(Some("foo/bar"));
        assert_eq!(url, "https://api.github.com/repos/foo/bar/releases/latest");
    }
}
