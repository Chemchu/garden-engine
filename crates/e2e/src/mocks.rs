use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::{STEAM_API_KEY, STEAM_ID, TestApp};

pub async fn mount_steam(app: &TestApp, owned_games: serde_json::Value, game_count: u32) {
    let ok = |body: serde_json::Value| ResponseTemplate::new(200).set_body_json(body);

    Mock::given(method("GET"))
        .and(path("/ISteamUser/GetPlayerSummaries/v2/"))
        .and(query_param("key", STEAM_API_KEY))
        .and(query_param("steamids", STEAM_ID))
        .respond_with(ok(json!({"response": {"players": [{
            "personaname": "FixtureGamer",
            "realname": "Fixture Real Name",
            "avatarfull": "https://avatars.example/full.jpg",
            "profileurl": "https://steamcommunity.com/id/fixture/",
            "steamid": STEAM_ID,
            "personastate": 1,
            "timecreated": 1_300_000_000_u64,
            "loccountrycode": "IE"
        }]}})))
        .mount(&app.steam)
        .await;
    Mock::given(method("GET"))
        .and(path("/IPlayerService/GetSteamLevel/v1/"))
        .respond_with(ok(json!({"response": {"player_level": 42}})))
        .mount(&app.steam)
        .await;
    Mock::given(method("GET"))
        .and(path("/IPlayerService/GetOwnedGames/v1/"))
        .respond_with(ok(
            json!({"response": {"game_count": game_count, "games": owned_games}}),
        ))
        .mount(&app.steam)
        .await;
    Mock::given(method("GET"))
        .and(path("/IPlayerService/GetRecentlyPlayedGames/v1/"))
        .and(query_param("count", "5"))
        .respond_with(ok(json!({"response": {"games": [{
            "name": "Recent Adventure", "appid": 20, "playtime_2weeks": 45,
            "playtime_forever": 90, "img_icon_url": "icon20"
        }]}})))
        .mount(&app.steam)
        .await;
    Mock::given(method("GET"))
        .and(path("/ISteamUserStats/GetPlayerAchievements/v1/"))
        .and(query_param("appid", "20"))
        .respond_with(ok(json!({"playerstats": {"achievements": [
            {"apiname": "A1", "achieved": 1, "unlocktime": 1_700_000_000_u64},
            {"apiname": "A2", "achieved": 0, "unlocktime": 0}
        ]}})))
        .mount(&app.steam)
        .await;
    Mock::given(method("GET"))
        .and(path("/ISteamUserStats/GetPlayerAchievements/v1/"))
        .respond_with(ResponseTemplate::new(400))
        .mount(&app.steam)
        .await;
}

pub async fn mount_font_catalog(app: &TestApp) {
    let font = |id: &str, family: &str, weights: &[u16], category: &str| {
        json!({
            "id": id,
            "family": family,
            "subsets": ["latin"],
            "weights": weights,
            "styles": ["normal"],
            "category": category,
            "type": "google",
        })
    };

    Mock::given(method("GET"))
        .and(path("/v1/fonts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            font("inter", "Inter", &[400, 600, 700], "sans-serif"),
            font("lora", "Lora", &[400, 700], "serif"),
            json!({
                "id": "iosevka",
                "family": "Iosevka",
                "subsets": ["latin"],
                "weights": [400, 600],
                "styles": ["normal"],
                "category": "monospace",
                "type": "other",
            }),
        ])))
        .mount(&app.fonts)
        .await;
}
