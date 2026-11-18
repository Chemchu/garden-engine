use e2e::TestApp;

const STARTER_GARDEN: &str = "starter/garden";

#[tokio::test]
async fn the_starter_garden_renders_with_every_link_resolved() {
    let app = TestApp::spawn_with_garden(STARTER_GARDEN).await;

    let home = app.get("/").await;
    assert_eq!(home.status, 200);
    assert!(home.body.contains("Welcome to my digital garden"));

    let spanish = app.get_in("/posts/welcome", "es").await;
    assert!(spanish.body.contains("Bienvenida a mi jardín digital"));

    for route in ["welcome", "notes/writing-posts", "notes/linking"] {
        let page = app.get(&format!("/posts/{route}")).await;
        assert_eq!(page.status, 200, "{route}");
        assert!(!page.body.contains("wikilink-unresolved"), "{route}");
    }

    let linking = app.get("/posts/notes/linking").await.body;
    assert!(linking.contains(r#"href="/posts/notes/writing-posts""#));
    assert!(linking.contains(r#"href="/posts/notes/writing-posts#drafts""#));
    assert!(linking.contains(r#"href="/posts/welcome""#));
    assert!(linking.contains("Each tag gets its own page"));
}
