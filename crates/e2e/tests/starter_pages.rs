use e2e::{Response, STARTER_GARDEN_PATH, TestApp};

fn post_routes() -> Vec<String> {
    let mut routes: Vec<String> = walkdir::WalkDir::new(STARTER_GARDEN_PATH)
        .into_iter()
        .map(|entry| entry.unwrap().into_path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .map(|path| {
            let relative = path.strip_prefix(STARTER_GARDEN_PATH).unwrap();
            relative.with_extension("").to_string_lossy().into_owned()
        })
        .collect();
    routes.sort();
    routes
}

fn assert_ok(path: &str, response: &Response) {
    assert_eq!(response.status, 200, "{path} failed:\n{}", response.body);
}

#[tokio::test]
async fn every_starter_post_renders_every_page() {
    let app = TestApp::spawn_with_garden(STARTER_GARDEN_PATH).await;
    let routes = post_routes();
    assert!(!routes.is_empty(), "the starter garden was not found");

    for route in &routes {
        for path in [
            format!("/posts/{route}"),
            format!("/metadata/{route}"),
            format!("/graph/{route}"),
            format!("/graph/{route}?depth=3"),
        ] {
            assert_ok(&path, &app.htmx_get(&path).await);
        }
        let full = format!("/posts/{route}");
        assert_ok(&full, &app.get(&full).await);
    }
}

#[tokio::test]
async fn every_starter_post_has_a_preview() {
    let app = TestApp::spawn_with_garden(STARTER_GARDEN_PATH).await;
    let switcher: serde_json::Value =
        serde_json::from_str(&app.get("/switcher.json").await.body).unwrap();

    for post in switcher.as_array().unwrap() {
        let path = format!("/preview/{}", post["path"].as_str().unwrap());
        assert_ok(&path, &app.get(&path).await);
    }
}

#[tokio::test]
async fn starter_listings_render() {
    let app = TestApp::spawn_with_garden(STARTER_GARDEN_PATH).await;

    for path in [
        "/",
        "/news",
        "/tags",
        "/rss.xml",
        "/switcher.json",
        "/garden-view",
        "/search?limit=100&view=topics",
        "/search?limit=100&view=folders",
        "/search?limit=100&query=links",
    ] {
        assert_ok(path, &app.htmx_get(path).await);
    }

    let tags = app.htmx_get("/tags").await.body;
    for tag_link in tags.split("href=\"/tags/").skip(1) {
        let tag = &tag_link[..tag_link.find('"').unwrap()];
        let path = format!("/tags/{tag}");
        assert_ok(&path, &app.htmx_get(&path).await);
    }
}
