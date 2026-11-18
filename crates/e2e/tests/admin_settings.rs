use e2e::{ADMIN_PASSWORD, ADMIN_USERNAME, TestApp, mocks};
use garden_engine::settings::{FontRequest, SettingsRequest};
use garden_engine::site_config::FontFallback;
use reqwest::{Client, StatusCode, Url, header, redirect::Policy};

const CURRENT: [(&str, &str); 21] = [
    ("site_name", "E2E garden"),
    (
        "site_source_url",
        "https://github.com/Chemchu/garden-engine",
    ),
    (
        "description_en",
        "My personal digital garden, built with Rust + HTMX",
    ),
    (
        "description_es",
        "Mi jardín digital personal, hecho con Rust + HTMX",
    ),
    (
        "site_links",
        "GITHUB https://www.github.com/Chemchu\r\nLINKEDIN https://www.linkedin.com/in/gustavo-lee",
    ),
    ("color_background", "#1e242c"),
    ("color_bright", "#dbdfe5"),
    ("color_primary", "#f58a07"),
    ("color_secondary", "#3f826d"),
    ("color_accent", "#bc6c25"),
    ("color_shade", "#323c49"),
    ("body_font", "keep"),
    ("body_weight", "600"),
    ("body_fallback", "sans-serif"),
    ("heading_font", "keep"),
    ("heading_weight", "600"),
    ("heading_fallback", "sans-serif"),
    ("feature_news", "on"),
    ("feature_chess", "on"),
    ("feature_music", "on"),
    ("feature_games", "on"),
];

struct Admin {
    client: Client,
    base: String,
}

impl Admin {
    async fn sign_in(app: &TestApp) -> Admin {
        let admin = Admin {
            client: Client::builder()
                .cookie_store(true)
                .redirect(Policy::none())
                .build()
                .unwrap(),
            base: app.address.clone(),
        };
        let response = admin
            .post(
                "/admin/login",
                &[("username", ADMIN_USERNAME), ("password", ADMIN_PASSWORD)],
            )
            .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        admin
    }

    async fn post(&self, path: &str, form: &[(&str, &str)]) -> reqwest::Response {
        self.client
            .post(format!("{}{path}", self.base))
            .header(header::ORIGIN, &self.base)
            .form(form)
            .send()
            .await
            .unwrap()
    }

    async fn settings_page(&self) -> String {
        let response = self
            .client
            .get(format!("{}/admin", self.base))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response.text().await.unwrap()
    }

    async fn submit(&self, changes: &[(&str, &str)], removed: &[&str]) -> reqwest::Response {
        let mut form: Vec<(&str, &str)> = CURRENT
            .iter()
            .copied()
            .filter(|(name, _)| !removed.contains(name))
            .map(|(name, value)| {
                changes
                    .iter()
                    .find(|(changed, _)| *changed == name)
                    .copied()
                    .unwrap_or((name, value))
            })
            .collect();
        form.extend(
            changes
                .iter()
                .filter(|(name, _)| !CURRENT.iter().any(|(current, _)| current == name)),
        );
        self.post("/admin/settings", &form).await
    }
}

fn requested_settings(response: &reqwest::Response) -> SettingsRequest {
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = Url::parse(response.headers()[header::LOCATION].to_str().unwrap()).unwrap();
    assert_eq!(location.host_str(), Some("github.com"));
    assert_eq!(location.path(), "/example/garden/issues/new");

    let query: Vec<(String, String)> = location.query_pairs().into_owned().collect();
    let value = |key: &str| {
        query
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
            .unwrap()
    };
    assert_eq!(value("title"), "Site settings update");
    assert_eq!(value("labels"), "site-settings");
    SettingsRequest::from_issue_body(&value("body")).unwrap()
}

#[tokio::test]
async fn the_settings_page_starts_from_the_current_site() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let page = Admin::sign_in(&app).await.settings_page().await;

    assert!(page.contains(r#"action="/admin/settings""#));
    assert!(page.contains(r##"name="color_primary" value="#f58a07""##));
    assert!(page.contains("Current: Iosevka"));
    assert!(page.contains("Current: Tusker Grotesk"));
    assert!(page.contains(r#"<option value="Inter">Google Fonts</option>"#));
    assert!(page.contains(r#"<option value="Iosevka">Fontsource, open source</option>"#));
    assert!(page.contains(r#"name="feature_chess" checked"#));
    assert!(page.contains(
        r#""id":"lora","family":"Lora","weights":[400,700],"category":"serif","subset":"latin""#
    ));
    assert!(page.contains(r#""id":"iosevka","family":"Iosevka","weights":[400,600],"category":"monospace","subset":"latin""#));
    assert!(page.contains(r#"<script src="/_static/admin-settings.js"></script>"#));
    assert!(page.contains(r#"data-font-picker="body""#));
    assert!(page.contains(r#"data-font-picker="heading""#));
    assert!(page.contains(r#"data-keep-label="Keep the current font (Tusker Grotesk)""#));
    assert!(page.contains(r#"id="settings-preview""#));
    assert!(page.contains(r#"data-specimen="body" data-kept-weights="600""#));
    assert!(page.contains(r#"data-font-cdn="https://cdn.jsdelivr.net/fontsource/fonts""#));
    assert!(!page.contains("font catalog is unavailable"));
}

#[tokio::test]
async fn changes_are_sent_to_github_as_an_issue() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin
        .submit(
            &[
                ("color_primary", "#ff0066"),
                ("body_font", "Inter"),
                ("heading_font", "lora"),
                ("heading_weight", "700"),
                ("heading_fallback", "serif"),
            ],
            &["feature_chess"],
        )
        .await;

    let request = requested_settings(&response);
    assert_eq!(request.colors.primary, "#ff0066");
    assert_eq!(request.colors.background, "#1e242c");
    assert_eq!(
        request.fonts.body,
        FontRequest::Fontsource {
            id: "inter".to_string(),
            weight: 600,
            fallback: FontFallback::SansSerif,
        }
    );
    assert_eq!(
        request.fonts.heading,
        FontRequest::Fontsource {
            id: "lora".to_string(),
            weight: 700,
            fallback: FontFallback::Serif,
        }
    );
    assert!(!request.features.chess);
    assert!(request.features.news && request.features.music);
    assert_eq!(request.identity.name, "E2E garden");
    assert_eq!(request.identity.links.len(), 2);
}

#[tokio::test]
async fn the_site_identity_can_be_changed() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let page = admin.settings_page().await;
    assert!(page.contains(r#"name="site_name" value="E2E garden""#));
    assert!(page.contains("GITHUB https://www.github.com/Chemchu\nLINKEDIN"));

    let response = admin
        .submit(
            &[
                ("site_name", "Ana's garden"),
                ("site_source_url", ""),
                ("description_es", ""),
                ("site_links", "MAIL mailto:ana@example.com"),
            ],
            &[],
        )
        .await;

    let request = requested_settings(&response);
    assert_eq!(request.identity.name, "Ana's garden");
    assert_eq!(request.identity.source_url, None);
    assert!(!request.identity.description.contains_key("es"));
    assert_eq!(request.identity.links[0].label, "MAIL");

    let invalid = admin
        .submit(&[("site_links", "EVIL javascript:alert(1)")], &[])
        .await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        invalid
            .text()
            .await
            .unwrap()
            .contains("site.links[0].url must be")
    );
}

#[tokio::test]
async fn open_source_fonts_can_be_chosen() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin
        .submit(
            &[
                ("heading_font", "Iosevka"),
                ("heading_weight", "600"),
                ("heading_fallback", "monospace"),
            ],
            &[],
        )
        .await;

    assert_eq!(
        requested_settings(&response).fonts.heading,
        FontRequest::Fontsource {
            id: "iosevka".to_string(),
            weight: 600,
            fallback: FontFallback::Monospace,
        }
    );
}

#[tokio::test]
async fn kept_fonts_can_change_their_fallback() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin
        .submit(
            &[("heading_fallback", "serif")],
            &["body_weight", "heading_weight"],
        )
        .await;

    let request = requested_settings(&response);
    assert_eq!(request.fonts.body, FontRequest::keep());
    assert_eq!(
        request.fonts.heading,
        FontRequest::Keep {
            fallback: Some(FontFallback::Serif),
        }
    );
}

#[tokio::test]
async fn new_fonts_without_a_weight_get_the_closest_one() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin
        .submit(&[("heading_font", "Lora")], &["heading_weight"])
        .await;

    assert_eq!(
        requested_settings(&response).fonts.heading,
        FontRequest::Fontsource {
            id: "lora".to_string(),
            weight: 700,
            fallback: FontFallback::SansSerif,
        }
    );
}

#[tokio::test]
async fn submitting_the_current_settings_changes_nothing() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin.submit(&[], &[]).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("Nothing has changed.")
    );
}

#[tokio::test]
async fn unknown_fonts_and_missing_weights_are_explained() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin
        .submit(
            &[
                ("body_font", "Comic <Sans>"),
                ("heading_font", "Lora"),
                ("heading_weight", "300"),
            ],
            &[],
        )
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let page = response.text().await.unwrap();
    assert!(
        page.contains("There is no font called &quot;Comic &lt;Sans&gt;&quot; in the catalog.")
    );
    assert!(page.contains("Lora has no weight 300."));
    assert!(page.contains(r#"value="Comic &lt;Sans&gt;""#));
}

#[tokio::test]
async fn the_corner_radius_is_sent_with_the_request() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let page = admin.settings_page().await;
    assert!(page.contains(r#"name="radius" min="0" max="24" step="1" value="0""#));
    assert!(page.contains(r#"<output id="radius-value" class="tabular-nums">0px</output>"#));

    let rounder = admin.submit(&[("radius", "8")], &[]).await;
    assert_eq!(requested_settings(&rounder).radius, Some(8));

    let unchanged = admin.submit(&[("radius", "0")], &[]).await;
    assert_eq!(unchanged.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let too_round = admin.submit(&[("radius", "25")], &[]).await;
    assert_eq!(too_round.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        too_round
            .text()
            .await
            .unwrap()
            .contains("radius must be between 0 and 24 pixels, got 25")
    );
}

#[tokio::test]
async fn invalid_colors_are_rejected() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin
        .submit(&[("color_primary", "red;}body{display:none")], &[])
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("colors.primary must be a #rrggbb color")
    );
}

#[tokio::test]
async fn without_the_catalog_only_the_current_fonts_can_be_kept() {
    let app = TestApp::spawn_with_admin().await;
    let admin = Admin::sign_in(&app).await;

    let page = admin.settings_page().await;
    assert!(page.contains("The font catalog is unavailable right now"));
    assert!(page.contains(r#"<script type="application/json" id="font-catalog-data">[]</script>"#));

    let new_font = admin.submit(&[("body_font", "Inter")], &[]).await;
    assert_eq!(new_font.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let colors_only = admin.submit(&[("color_accent", "#123456")], &[]).await;
    assert_eq!(requested_settings(&colors_only).colors.accent, "#123456");
}

#[tokio::test]
async fn settings_need_a_session_and_the_same_origin() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;

    let anonymous = Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap()
        .post(format!("{}/admin/settings", app.address))
        .form(&CURRENT)
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::SEE_OTHER);
    assert_eq!(anonymous.headers()[header::LOCATION], "/admin/login");

    let admin = Admin::sign_in(&app).await;
    let cross_site = admin
        .client
        .post(format!("{}/admin/settings", app.address))
        .header(header::ORIGIN, "https://evil.example")
        .form(&CURRENT)
        .send()
        .await
        .unwrap();
    assert_eq!(cross_site.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn reset_buttons_bring_back_the_deployed_values_after_a_failed_submit() {
    let app = TestApp::spawn_with_admin().await;
    mocks::mount_font_catalog(&app).await;
    let admin = Admin::sign_in(&app).await;

    let response = admin
        .submit(
            &[
                ("site_name", "Typed name"),
                ("description_en", "Typed description"),
                ("color_primary", "not-a-color"),
                ("color_accent", "#abcdef"),
                ("body_font", "Lora"),
                ("body_weight", "700"),
                ("body_fallback", "serif"),
                ("radius", "12"),
            ],
            &["feature_chess"],
        )
        .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let page = response.text().await.unwrap();

    assert!(page.contains(r#"data-reset-group="styles""#));
    assert!(page.contains(r#"data-reset-group="details""#));
    for fragment in [
        r##"value="Typed name" data-reset="details" data-deployed="E2E garden""##,
        r##"value="Typed description" data-reset="details" data-deployed="My personal digital garden, built with Rust + HTMX""##,
        r##"value="#abcdef" data-color="accent" data-reset="styles" data-deployed="#bc6c25""##,
        r##"value="Lora" list="font-catalog" data-font-role="body" data-reset="styles" data-deployed="keep""##,
        r##"name="body_weight" data-font-role="body" data-reset="styles" data-deployed="600""##,
        r##"name="body_fallback" data-font-role="body" data-reset="styles" data-deployed="sans-serif""##,
        r##"name="feature_chess" data-reset="details" data-deployed="true""##,
        r##"name="radius" min="0" max="24" step="1" value="12" data-radius data-reset="styles" data-deployed="0""##,
    ] {
        assert!(page.contains(fragment), "{fragment}");
    }
    assert!(page.contains(r#"data-deployed="GITHUB https://www.github.com/Chemchu"#));
}
