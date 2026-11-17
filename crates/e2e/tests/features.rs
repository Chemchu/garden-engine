use e2e::TestApp;
use guslee::site_config::Features;

const FEATURE_ROUTES: [&str; 6] = [
    "/news",
    "/chess/stats/blitz",
    "/chess/games/analysis",
    "/music/profile",
    "/steam",
    "/schedule",
];

const NONE: Features = Features {
    news: false,
    chess: false,
    music: false,
    games: false,
    schedule: false,
};

#[tokio::test]
async fn disabled_features_have_no_routes_and_no_links() {
    let app = TestApp::spawn_with_features(NONE).await;

    for path in FEATURE_ROUTES {
        assert_eq!(app.htmx_get(path).await.status, 404, "{path}");
    }

    let home = app.get("/posts/welcome").await;
    assert_eq!(home.status, 200);
    for path in FEATURE_ROUTES {
        assert!(!home.body.contains(&format!("href=\"{path}")), "{path}");
        assert!(!home.body.contains(&format!("hx-get=\"{path}")), "{path}");
    }
    assert!(home.body.contains("href=\"/tags\""));
    assert!(home.body.contains("href=\"/garden-view-dispatcher\""));

    assert_eq!(app.htmx_get("/tags").await.status, 200);
}

#[tokio::test]
async fn features_can_be_switched_off_one_at_a_time() {
    let app = TestApp::spawn_with_features(Features {
        music: false,
        ..Features::default()
    })
    .await;

    assert_eq!(app.htmx_get("/music/profile").await.status, 404);
    assert_eq!(app.htmx_get("/schedule").await.status, 200);

    let home = app.get("/posts/welcome").await.body;
    assert!(!home.contains("/music/profile"));
    assert!(home.contains("href=\"/chess/stats/blitz\""));
    assert!(home.contains("href=\"/steam\""));
    assert!(home.contains("href=\"/news\""));
}

#[tokio::test]
async fn sections_without_credentials_are_switched_off() {
    let app = TestApp::spawn_without_integrations().await;

    for path in ["/chess/stats/blitz", "/music/profile", "/steam"] {
        assert_eq!(app.htmx_get(path).await.status, 404, "{path}");
    }
    assert_eq!(app.htmx_get("/news").await.status, 200);
    assert_eq!(app.htmx_get("/schedule").await.status, 200);

    let home = app.get("/posts/welcome").await.body;
    assert!(!home.contains("chess-link") && !home.contains("music-link"));
    assert!(!home.contains("games-link"));
    assert!(home.contains("news-section"));
}
