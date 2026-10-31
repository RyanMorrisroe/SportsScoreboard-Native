use std::{sync::OnceLock, time::Duration};

use reqwest::{Client, StatusCode, Url, redirect::Policy};
use serde::{Deserialize, Serialize};

const RELEASES_URL: &str =
    "https://api.github.com/repos/RyanMorrisroe/SportsScoreboard-Native/releases?per_page=100";
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RELEASE_NOTES_CHARS: usize = 4000;

static GITHUB_CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Platform {
    Android,
    Windows,
}

impl Platform {
    pub(super) fn current() -> Result<Self, String> {
        if cfg!(target_os = "android") {
            Ok(Self::Android)
        } else if cfg!(target_os = "windows") {
            Ok(Self::Windows)
        } else {
            Err("Update checks are not supported on this platform".to_string())
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppUpdate {
    version: String,
    notes: String,
    download_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CompositeVersion([u64; 6]);

fn parse_three_numbers(value: &str) -> Option<[u64; 3]> {
    let mut parts = value.split('.');
    let values = [parts.next()?, parts.next()?, parts.next()?];
    if parts.next().is_some() {
        return None;
    }

    let mut parsed = [0; 3];
    for (index, part) in values.into_iter().enumerate() {
        if part.is_empty()
            || (part.len() > 1 && part.starts_with('0'))
            || !part.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        parsed[index] = part.parse().ok()?;
    }
    Some(parsed)
}

fn parse_composite_version(value: &str) -> Option<CompositeVersion> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let (app_version, wrapper_version) = value.split_once("-wrapper.")?;
    let app = parse_three_numbers(app_version)?;
    let wrapper = parse_three_numbers(wrapper_version)?;
    Some(CompositeVersion([
        app[0], app[1], app[2], wrapper[0], wrapper[1], wrapper[2],
    ]))
}

fn asset_matches(name: &str, platform: Platform) -> bool {
    match platform {
        Platform::Android => name.ends_with(".apk"),
        Platform::Windows => name.ends_with("-setup.exe"),
    }
}

fn valid_asset_url(value: &str, tag: &str, name: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    let expected_path =
        format!("/RyanMorrisroe/SportsScoreboard-Native/releases/download/{tag}/{name}");

    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path() == expected_path
}

fn select_update(
    releases: Vec<GitHubRelease>,
    current_version: &str,
    platform: Platform,
) -> Result<Option<AppUpdate>, String> {
    let current_version = parse_composite_version(current_version)
        .ok_or_else(|| "The installed app version is invalid".to_string())?;
    let latest = releases
        .into_iter()
        .filter(|release| !release.draft && !release.prerelease)
        .filter_map(|release| {
            parse_composite_version(&release.tag_name).map(|version| (version, release))
        })
        .max_by_key(|(version, _)| *version)
        .ok_or_else(|| "No published app release was found".to_string())?;

    if latest.0 <= current_version {
        return Ok(None);
    }

    let release = latest.1;
    let mut matching_assets = release
        .assets
        .into_iter()
        .filter(|asset| asset_matches(&asset.name, platform));
    let asset = matching_assets
        .next()
        .ok_or_else(|| "No compatible installer was found in the latest release".to_string())?;
    if matching_assets.next().is_some()
        || !valid_asset_url(&asset.browser_download_url, &release.tag_name, &asset.name)
    {
        return Err("The latest release has an invalid installer URL".to_string());
    }

    let notes = release
        .body
        .unwrap_or_default()
        .chars()
        .take(MAX_RELEASE_NOTES_CHARS)
        .collect();
    Ok(Some(AppUpdate {
        version: release.tag_name.trim_start_matches('v').to_string(),
        notes,
        download_url: asset.browser_download_url,
    }))
}

fn validate_response(status: StatusCode, content_length: Option<u64>) -> Result<(), String> {
    if !status.is_success() {
        return Err(format!("GitHub release request failed ({status})"));
    }
    if content_length.is_some_and(|length| length > MAX_RESPONSE_BYTES as u64) {
        return Err("GitHub release response exceeded the size limit".to_string());
    }
    Ok(())
}

fn append_response_chunk(body: &mut Vec<u8>, chunk: &[u8]) -> Result<(), String> {
    if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
        return Err("GitHub release response exceeded the size limit".to_string());
    }
    body.extend_from_slice(chunk);
    Ok(())
}

fn github_client() -> Result<&'static Client, String> {
    GITHUB_CLIENT
        .get_or_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("SportsScoreboard-Native")
                .redirect(Policy::none())
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(super) async fn check_for_update(
    current_version: &str,
    platform: Platform,
) -> Result<Option<AppUpdate>, String> {
    let response = github_client()?
        .get(RELEASES_URL)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|error| format!("GitHub release request failed: {error}"))?;
    let status = response.status();
    let content_length = response.content_length();
    validate_response(status, content_length)?;

    let mut body = Vec::new();
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("GitHub release request failed: {error}"))?
    {
        append_response_chunk(&mut body, &chunk)?;
    }

    let releases = serde_json::from_slice::<Vec<GitHubRelease>>(&body)
        .map_err(|error| format!("Invalid GitHub release response: {error}"))?;
    select_update(releases, current_version, platform)
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::{
        AppUpdate, GitHubRelease, MAX_RESPONSE_BYTES, Platform, append_response_chunk,
        parse_composite_version, select_update, valid_asset_url, validate_response,
    };

    fn release(tag: &str, name: &str, url: &str) -> GitHubRelease {
        serde_json::from_value(serde_json::json!({
            "tag_name": tag,
            "body": "Release notes",
            "assets": [{ "name": name, "browser_download_url": url }]
        }))
        .unwrap()
    }

    #[test]
    fn parses_and_compares_composite_versions_numerically() {
        assert!(
            parse_composite_version("v1.10.0-wrapper.0.1.0")
                > parse_composite_version("1.9.99-wrapper.9.9.9")
        );
        assert!(
            parse_composite_version("1.7.1-wrapper.0.2.0")
                > parse_composite_version("1.7.1-wrapper.0.1.9")
        );
        assert!(parse_composite_version("v01.7.1-wrapper.0.1.0").is_none());
        assert!(parse_composite_version("1.7.1-wrapper.0.1").is_none());
        assert!(parse_composite_version("1.7.1").is_none());
    }

    #[test]
    fn chooses_the_highest_version_and_platform_asset() {
        let releases = vec![
            release(
                "v1.9.0-wrapper.0.1.0",
                "scoreboard-setup.exe",
                "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.9.0-wrapper.0.1.0/scoreboard-setup.exe",
            ),
            release(
                "v1.10.0-wrapper.0.1.0",
                "app-universal-release.apk",
                "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.10.0-wrapper.0.1.0/app-universal-release.apk",
            ),
        ];

        let android = select_update(releases, "1.8.0-wrapper.0.1.0", Platform::Android)
            .unwrap()
            .unwrap();
        assert_eq!(android.version, "1.10.0-wrapper.0.1.0");
        assert!(android.download_url.ends_with("app-universal-release.apk"));

        let windows_release: GitHubRelease = serde_json::from_value(serde_json::json!({
            "tag_name": "v1.10.0-wrapper.0.1.0",
            "body": "Release notes",
            "assets": [
                {
                    "name": "sports-scoreboard-setup.exe",
                    "browser_download_url": "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.10.0-wrapper.0.1.0/sports-scoreboard-setup.exe"
                },
                {
                    "name": "app-universal-release.apk",
                    "browser_download_url": "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.10.0-wrapper.0.1.0/app-universal-release.apk"
                }
            ]
        }))
        .unwrap();
        let windows = select_update(
            vec![windows_release],
            "1.8.0-wrapper.0.1.0",
            Platform::Windows,
        )
        .unwrap()
        .unwrap();
        assert!(windows.download_url.ends_with("-setup.exe"));
    }

    #[test]
    fn returns_no_update_for_installed_or_older_versions() {
        let releases = vec![release(
            "v1.7.1-wrapper.0.1.0",
            "app.apk",
            "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.7.1-wrapper.0.1.0/app.apk",
        )];
        assert!(
            select_update(releases, "1.7.1-wrapper.0.1.0", Platform::Android)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rejects_missing_or_ambiguous_platform_assets() {
        let no_asset = release(
            "v1.8.0-wrapper.0.1.0",
            "app.apk",
            "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.8.0-wrapper.0.1.0/app.apk",
        );
        assert!(select_update(vec![no_asset], "1.7.1-wrapper.0.1.0", Platform::Windows).is_err());

        let mut ambiguous = release(
            "v1.8.0-wrapper.0.1.0",
            "app.apk",
            "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.8.0-wrapper.0.1.0/app.apk",
        );
        ambiguous.assets.push(serde_json::from_value(serde_json::json!({
            "name": "second.apk",
            "browser_download_url": "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.8.0-wrapper.0.1.0/second.apk"
        })).unwrap());
        assert!(select_update(vec![ambiguous], "1.7.1-wrapper.0.1.0", Platform::Android).is_err());
    }

    #[test]
    fn rejects_download_urls_outside_the_exact_github_release_path() {
        assert!(!valid_asset_url(
            "https://evil.example/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.8.0-wrapper.0.1.0/app.apk",
            "v1.8.0-wrapper.0.1.0",
            "app.apk",
        ));
        assert!(!valid_asset_url(
            "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.8.0-wrapper.0.1.0/app.apk?redirect=1",
            "v1.8.0-wrapper.0.1.0",
            "app.apk",
        ));
        assert!(valid_asset_url(
            "https://github.com/RyanMorrisroe/SportsScoreboard-Native/releases/download/v1.8.0-wrapper.0.1.0/app.apk",
            "v1.8.0-wrapper.0.1.0",
            "app.apk",
        ));
    }

    #[test]
    fn bounds_release_response_status_and_size() {
        assert!(validate_response(StatusCode::OK, Some(MAX_RESPONSE_BYTES as u64)).is_ok());
        assert!(validate_response(StatusCode::FORBIDDEN, None).is_err());
        assert!(validate_response(StatusCode::OK, Some(MAX_RESPONSE_BYTES as u64 + 1)).is_err());

        let mut body = Vec::new();
        assert!(append_response_chunk(&mut body, &vec![0; MAX_RESPONSE_BYTES]).is_ok());
        assert!(append_response_chunk(&mut body, &[0]).is_err());
    }

    #[test]
    fn serializes_frontend_update_fields_in_camel_case() {
        let update = AppUpdate {
            version: "1.8.0-wrapper.0.1.0".to_string(),
            notes: "Release notes".to_string(),
            download_url: "https://github.com/example/release.apk".to_string(),
        };
        assert_eq!(
            serde_json::to_value(update).unwrap()["downloadUrl"],
            "https://github.com/example/release.apk"
        );
    }
}
