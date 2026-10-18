use std::{collections::HashMap, future::Future};

use reqwest::{StatusCode, Url};

pub(super) const MAX_ESPN_URL_LENGTH: usize = 2048;
pub(super) const MAX_ESPN_RESPONSE_BYTES: usize = 5 * 1024 * 1024;

fn valid_numeric_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 20 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_sport_league(sport: &str, league: &str) -> bool {
    matches!(
        (sport, league),
        ("football", "college-football")
            | ("football", "nfl")
            | ("baseball", "mlb")
            | ("basketball", "nba")
            | ("hockey", "nhl")
    )
}

fn parse_query(url: &Url) -> Option<HashMap<String, String>> {
    if url.query() == Some("") {
        return None;
    }

    let mut params = HashMap::new();
    for (key, value) in url.query_pairs() {
        if params
            .insert(key.into_owned(), value.into_owned())
            .is_some()
        {
            return None;
        }
    }
    Some(params)
}

fn valid_scoreboard_query(league: &str, params: &HashMap<String, String>) -> bool {
    match league {
        "college-football" => params.iter().all(|(key, value)| {
            matches!(key.as_str(), "week" | "seasontype" | "groups") && valid_numeric_id(value)
        }),
        "nfl" => params.iter().all(|(key, value)| {
            matches!(key.as_str(), "week" | "seasontype") && valid_numeric_id(value)
        }),
        "mlb" | "nba" | "nhl" => params
            .iter()
            .all(|(key, value)| key == "dates" && value.len() == 8 && valid_numeric_id(value)),
        _ => false,
    }
}

pub(super) fn is_allowed_espn_url(url: &Url) -> bool {
    if url.as_str().len() > MAX_ESPN_URL_LENGTH
        || url.scheme() != "https"
        || url.host_str() != Some("site.api.espn.com")
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return false;
    }

    let segments: Vec<_> = url.path_segments().into_iter().flatten().collect();
    let Some(params) = parse_query(url) else {
        return false;
    };

    if segments.len() >= 6 && segments[..4] == ["apis", "site", "v2", "sports"] {
        let sport = segments[4];
        let league = segments[5];
        if !valid_sport_league(sport, league) {
            return false;
        }

        return match segments.as_slice() {
            ["apis", "site", "v2", "sports", _, _, "scoreboard"] => {
                valid_scoreboard_query(league, &params)
            }
            ["apis", "site", "v2", "sports", _, _, "summary"] => {
                params.len() == 1
                    && params
                        .get("event")
                        .is_some_and(|event_id| valid_numeric_id(event_id))
            }
            ["apis", "site", "v2", "sports", _, _, "teams", team_id]
            | [
                "apis",
                "site",
                "v2",
                "sports",
                _,
                _,
                "teams",
                team_id,
                "schedule",
            ] => params.is_empty() && valid_numeric_id(team_id),
            _ => false,
        };
    }

    match segments.as_slice() {
        ["apis", "v2", "sports", sport, league, "standings"]
            if valid_sport_league(sport, league) =>
        {
            params.is_empty()
                || (params.len() == 1
                    && params
                        .get("group")
                        .is_some_and(|group_id| valid_numeric_id(group_id)))
        }
        _ => false,
    }
}

pub(super) fn parse_espn_url(value: &str) -> Result<Url, String> {
    if value.len() > MAX_ESPN_URL_LENGTH {
        return Err("ESPN URL exceeded the size limit".to_string());
    }

    let url = Url::parse(value).map_err(|_| "Invalid ESPN URL".to_string())?;
    if !is_allowed_espn_url(&url) {
        return Err("Unsupported ESPN URL".to_string());
    }
    Ok(url)
}

pub(super) fn should_follow_redirect(url: &Url, previous_redirects: usize) -> bool {
    previous_redirects < 5 && is_allowed_espn_url(url)
}

pub(super) fn validate_response_metadata(
    status: StatusCode,
    content_length: Option<u64>,
) -> Result<(), String> {
    if !status.is_success() {
        return Err(format!("ESPN request failed ({status})"));
    }
    if content_length.is_some_and(|length| length > MAX_ESPN_RESPONSE_BYTES as u64) {
        return Err("ESPN response exceeded the size limit".to_string());
    }
    Ok(())
}

pub(super) fn append_response_chunk(body: &mut Vec<u8>, chunk: &[u8]) -> Result<(), String> {
    if body.len().saturating_add(chunk.len()) > MAX_ESPN_RESPONSE_BYTES {
        return Err("ESPN response exceeded the size limit".to_string());
    }
    body.extend_from_slice(chunk);
    Ok(())
}

pub(super) fn parse_espn_response(
    status: StatusCode,
    content_length: Option<u64>,
    body: &[u8],
) -> Result<serde_json::Value, String> {
    validate_response_metadata(status, content_length)?;
    if body.len() > MAX_ESPN_RESPONSE_BYTES {
        return Err("ESPN response exceeded the size limit".to_string());
    }
    serde_json::from_slice(body).map_err(|error| format!("Invalid ESPN JSON: {error}"))
}

pub(super) struct EspnResponse {
    pub(super) status: StatusCode,
    pub(super) content_length: Option<u64>,
    pub(super) body: Vec<u8>,
}

pub(super) async fn fetch_espn_with<F, Fut>(
    value: &str,
    request: F,
) -> Result<serde_json::Value, String>
where
    F: FnOnce(Url) -> Fut,
    Fut: Future<Output = Result<EspnResponse, String>>,
{
    let url = parse_espn_url(value)?;
    let response = request(url)
        .await
        .map_err(|error| format!("ESPN request failed: {error}"))?;
    parse_espn_response(response.status, response.content_length, &response.body)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, future::ready};

    use reqwest::{StatusCode, Url};

    use super::{
        EspnResponse, MAX_ESPN_RESPONSE_BYTES, MAX_ESPN_URL_LENGTH, append_response_chunk,
        fetch_espn_with, is_allowed_espn_url, parse_espn_response, parse_espn_url,
        should_follow_redirect, valid_numeric_id, valid_scoreboard_query,
    };

    #[test]
    fn validates_numeric_identifiers_and_scoreboard_leagues() {
        assert!(valid_numeric_id("0"));
        assert!(valid_numeric_id("12345678901234567890"));
        assert!(!valid_numeric_id(""));
        assert!(!valid_numeric_id("123456789012345678901"));
        assert!(!valid_numeric_id("12x"));
        assert!(!valid_scoreboard_query("unsupported", &HashMap::new()));
    }

    #[test]
    fn allows_supported_espn_routes() {
        let urls = [
            "https://site.api.espn.com/apis/site/v2/sports/football/college-football/scoreboard?week=3&seasontype=2&groups=80",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard?week=3&seasontype=2",
            "https://site.api.espn.com/apis/site/v2/sports/baseball/mlb/scoreboard?dates=20261017",
            "https://site.api.espn.com/apis/site/v2/sports/basketball/nba/scoreboard",
            "https://site.api.espn.com/apis/site/v2/sports/hockey/nhl/scoreboard?dates=20261017",
            "https://site.api.espn.com/apis/site/v2/sports/football/college-football/summary?event=401234567",
            "https://site.api.espn.com/apis/site/v2/sports/basketball/nba/summary?event=401234567",
            "https://site.api.espn.com/apis/site/v2/sports/baseball/mlb/teams/12",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/teams/12/schedule",
            "https://site.api.espn.com/apis/site/v2/sports/hockey/nhl/teams/12",
            "https://site.api.espn.com/apis/site/v2/sports/hockey/nhl/teams/12/schedule",
            "https://site.api.espn.com/apis/v2/sports/baseball/mlb/standings",
            "https://site.api.espn.com/apis/v2/sports/football/nfl/standings?group=8",
        ];

        for value in urls {
            assert!(is_allowed_espn_url(&Url::parse(value).unwrap()), "{value}");
        }
    }

    #[test]
    fn rejects_unsupported_espn_urls() {
        let urls = [
            "http://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard",
            "https://example.com/apis/site/v2/sports/football/nfl/scoreboard",
            "https://site.api.espn.com.evil.test/apis/site/v2/sports/football/nfl/scoreboard",
            "https://user@site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard",
            "https://site.api.espn.com:444/apis/site/v2/sports/football/nfl/scoreboard",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard#fragment",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard?",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard?unknown=1",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard?week=1&week=2",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard?week=",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard?dates=20261017",
            "https://site.api.espn.com/apis/site/v2/sports/football/college-football/scoreboard?dates=20261017",
            "https://site.api.espn.com/apis/site/v2/sports/baseball/mlb/scoreboard?dates=2026101x",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/summary",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/summary?event=abc",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/summary?event=123&extra=1",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/teams/not-a-number",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/teams/12?foo=bar",
            "https://site.api.espn.com/apis/site/v2/sports/football/nfl/teams/12/extra",
            "https://site.api.espn.com/apis/v2/sports/football/nfl/standings?group=abc",
            "https://site.api.espn.com/apis/v2/sports/football/nfl/standings?group=1&group=2",
            "https://site.api.espn.com/apis/site/v2/sports/football/unknown/scoreboard",
            "https://site.api.espn.com/apis/unknown/sports/football/nfl/standings",
        ];

        for value in urls {
            assert!(!is_allowed_espn_url(&Url::parse(value).unwrap()), "{value}");
        }
    }

    #[test]
    fn validates_urls_before_network_use() {
        assert_eq!(
            parse_espn_url("https://site.api.espn.com/apis/site/v2/sports/nba/teams/12"),
            Err("Unsupported ESPN URL".to_string())
        );
        assert_eq!(
            parse_espn_url("not a URL"),
            Err("Invalid ESPN URL".to_string())
        );
        assert_eq!(
            parse_espn_url(&"a".repeat(MAX_ESPN_URL_LENGTH + 1)),
            Err("ESPN URL exceeded the size limit".to_string())
        );
    }

    #[test]
    fn follows_only_supported_redirects_with_a_limit() {
        let allowed =
            Url::parse("https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard")
                .unwrap();
        let external = Url::parse("https://example.com/").unwrap();

        assert!(should_follow_redirect(&allowed, 0));
        assert!(!should_follow_redirect(&allowed, 5));
        assert!(!should_follow_redirect(&external, 0));
    }

    #[test]
    fn decodes_successful_response_and_rejects_bad_responses() {
        assert_eq!(
            parse_espn_response(StatusCode::OK, Some(11), br#"{"ok":true}"#).unwrap(),
            serde_json::json!({"ok": true})
        );
        assert!(
            parse_espn_response(StatusCode::SERVICE_UNAVAILABLE, None, b"{}")
                .unwrap_err()
                .contains("503")
        );
        assert_eq!(
            parse_espn_response(
                StatusCode::OK,
                Some((MAX_ESPN_RESPONSE_BYTES + 1) as u64),
                b"{}"
            ),
            Err("ESPN response exceeded the size limit".to_string())
        );
        assert_eq!(
            parse_espn_response(
                StatusCode::OK,
                None,
                &vec![b'x'; MAX_ESPN_RESPONSE_BYTES + 1]
            ),
            Err("ESPN response exceeded the size limit".to_string())
        );
        assert!(
            parse_espn_response(StatusCode::OK, None, b"not json")
                .unwrap_err()
                .starts_with("Invalid ESPN JSON:")
        );
    }

    #[test]
    fn limits_streamed_body_chunks() {
        let mut body = vec![b'x'; MAX_ESPN_RESPONSE_BYTES - 1];
        append_response_chunk(&mut body, b"y").unwrap();
        assert_eq!(body.len(), MAX_ESPN_RESPONSE_BYTES);
        assert_eq!(
            append_response_chunk(&mut body, b"z"),
            Err("ESPN response exceeded the size limit".to_string())
        );
    }

    #[test]
    fn fetch_flow_validates_before_request_and_normalizes_transport_errors() {
        tauri::async_runtime::block_on(async {
            let mut requested = false;
            let invalid = fetch_espn_with("https://example.com/", |_| {
                requested = true;
                ready(Ok(EspnResponse {
                    status: StatusCode::OK,
                    content_length: Some(2),
                    body: b"{}".to_vec(),
                }))
            })
            .await;
            assert_eq!(invalid, Err("Unsupported ESPN URL".to_string()));
            assert!(!requested);

            let failed = fetch_espn_with(
                "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard",
                |_| ready(Err("offline".to_string())),
            )
            .await;
            assert_eq!(failed, Err("ESPN request failed: offline".to_string()));

            let success = fetch_espn_with(
                "https://site.api.espn.com/apis/site/v2/sports/football/nfl/scoreboard",
                |_| {
                    ready(Ok(EspnResponse {
                        status: StatusCode::OK,
                        content_length: Some(11),
                        body: br#"{"ok":true}"#.to_vec(),
                    }))
                },
            )
            .await;
            assert_eq!(success.unwrap(), serde_json::json!({"ok": true}));
        });
    }
}
