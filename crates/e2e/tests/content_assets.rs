use e2e::TestApp;

#[tokio::test]
async fn content_assets_are_served_before_the_engine_files() {
    let assets = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(assets.path().join("fonts/mine")).unwrap();
    std::fs::write(assets.path().join("fonts/mine/regular.woff2"), "wOF2 mine").unwrap();
    std::fs::write(assets.path().join("favicon.ico"), "content favicon").unwrap();
    std::fs::write(assets.path().join(".secret"), "hidden").unwrap();
    let app = TestApp::spawn_with_assets(assets.path()).await;

    let font = app.get("/_static/fonts/mine/regular.woff2").await;
    assert_eq!(font.status, 200);
    assert_eq!(font.body, "wOF2 mine");

    assert_eq!(
        app.get("/_static/favicon.ico").await.body,
        "content favicon"
    );

    let engine_script = app.get("/_static/htmx.min.js").await;
    assert_eq!(engine_script.status, 200);
    assert!(engine_script.body.contains("htmx"));

    let engine_font = app
        .get("/_static/fonts/iosevka/Iosevka-Regular-subset.woff2")
        .await;
    assert_eq!(engine_font.status, 200);

    assert_eq!(app.get("/_static/missing.js").await.status, 404);
    let hidden = app.get("/_static/.secret").await;
    assert_ne!(hidden.status, 200);
    assert!(!hidden.body.contains("hidden"));
    for path in [
        "/_static/../Cargo.toml",
        "/_static/%2e%2e/Cargo.toml",
        "/_static/fonts/../../site.toml",
    ] {
        let (status, body) = app.raw_get(path).await;
        assert_ne!(status, 200, "{path}");
        assert!(
            !body.contains("[workspace]") && !body.contains("[theme"),
            "{path}"
        );
    }
}

#[tokio::test]
async fn without_content_assets_only_engine_files_are_served() {
    let app = TestApp::spawn().await;

    assert_eq!(app.get("/_static/htmx.min.js").await.status, 200);
    assert_eq!(app.get("/_static/missing.js").await.status, 404);
}
