//! Asks GitHub for the latest release and says whether it is newer than this build.

use std::time::Duration;

const LATEST: &str =
    "https://api.github.com/repos/Abdulkader-Safi/SiteVet-DesktopAPP/releases/latest";

#[derive(Clone, Debug)]
pub struct Update {
    /// `0.2.0`, without the `v`.
    pub version: String,
    /// The installer for this OS, or the release page when the release has none.
    pub url: String,
}

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// `v0.2.10` or `0.2.10` as numbers; anything after the third number is ignored.
fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version
        .trim()
        .trim_start_matches('v')
        .split(['.', '-', '+']);
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

fn newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

/// The file name ending cargo-packager gives this OS's installer.
fn installer_suffix() -> &'static str {
    if cfg!(target_os = "macos") {
        ".dmg"
    } else if cfg!(windows) {
        "-setup.exe"
    } else {
        ".AppImage"
    }
}

/// Blocking: run it off the UI thread. Any network or parse failure reads as "no update",
/// so an offline start shows nothing.
pub fn check() -> Option<Update> {
    let body = reqwest::blocking::Client::builder()
        .user_agent(concat!("sitevet-desktop/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .send()
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .ok()?;
    let release: Release = serde_json::from_str(&body).ok()?;
    pick(release, env!("CARGO_PKG_VERSION"))
}

fn pick(release: Release, current: &str) -> Option<Update> {
    if release.draft || release.prerelease || !newer(&release.tag_name, current) {
        return None;
    }
    let url = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(installer_suffix()))
        .map_or(release.html_url, |a| a.browser_download_url.clone());
    Some(Update {
        version: release.tag_name.trim_start_matches('v').to_string(),
        url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_as_numbers() {
        assert!(newer("v0.2.0", "0.1.9"));
        assert!(newer("v0.1.10", "0.1.9"));
        assert!(!newer("v0.1.0", "0.1.0"));
        assert!(!newer("v0.0.9", "0.1.0"));
        assert!(!newer("nightly", "0.1.0"));
    }

    #[test]
    fn picks_this_os_installer() {
        let release = Release {
            tag_name: "v9.0.0".into(),
            html_url: "https://example.com/release".into(),
            draft: false,
            prerelease: false,
            assets: [
                "SiteVet_9.0.0_aarch64.dmg",
                "SiteVet_9.0.0_x64-setup.exe",
                "SiteVet_9.0.0_amd64.AppImage",
            ]
            .into_iter()
            .map(|n| Asset {
                name: n.into(),
                browser_download_url: format!("https://example.com/{n}"),
            })
            .collect(),
        };
        let update = pick(release, "0.1.0").unwrap();
        assert_eq!(update.version, "9.0.0");
        assert!(update.url.ends_with(installer_suffix()));
    }

    #[test]
    fn skips_prereleases() {
        let release = Release {
            tag_name: "v9.0.0".into(),
            html_url: String::new(),
            draft: false,
            prerelease: true,
            assets: vec![],
        };
        assert!(pick(release, "0.1.0").is_none());
    }
}
