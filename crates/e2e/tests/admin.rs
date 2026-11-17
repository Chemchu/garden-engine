use e2e::{ADMIN_PASSWORD, ADMIN_USERNAME, TestApp};
use guslee::controllers::admin_controller::{MAX_ATTEMPTS_PER_CLIENT, SESSION_COOKIE};
use reqwest::{Client, StatusCode, header, redirect::Policy};

struct Browser {
    client: Client,
    base: String,
}

impl Browser {
    fn new(app: &TestApp) -> Browser {
        Browser {
            client: Client::builder()
                .cookie_store(true)
                .redirect(Policy::none())
                .build()
                .unwrap(),
            base: app.address.clone(),
        }
    }

    async fn get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{path}", self.base))
            .send()
            .await
            .unwrap()
    }

    async fn post(&self, path: &str, form: &[(&str, &str)]) -> reqwest::Response {
        self.post_from(path, form, &self.base).await
    }

    async fn post_from(
        &self,
        path: &str,
        form: &[(&str, &str)],
        origin: &str,
    ) -> reqwest::Response {
        self.client
            .post(format!("{}{path}", self.base))
            .header(header::ORIGIN, origin)
            .form(form)
            .send()
            .await
            .unwrap()
    }

    async fn sign_in(&self, password: &str) -> reqwest::Response {
        self.post(
            "/admin/login",
            &[("username", ADMIN_USERNAME), ("password", password)],
        )
        .await
    }
}

fn location(response: &reqwest::Response) -> &str {
    response.headers()[header::LOCATION].to_str().unwrap()
}

#[tokio::test]
async fn the_admin_area_does_not_exist_without_admin_variables() {
    let app = TestApp::spawn().await;
    let browser = Browser::new(&app);

    assert_eq!(
        browser.get("/admin/login").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(browser.get("/admin").await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_login_page_is_served_with_strict_headers() {
    let app = TestApp::spawn_with_admin().await;
    let response = Browser::new(&app).get("/admin/login").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::X_FRAME_OPTIONS], "DENY");
    let body = response.text().await.unwrap();
    assert!(body.starts_with("<!doctype html>"));
    assert!(body.contains(r#"action="/admin/login""#));
}

#[tokio::test]
async fn the_dashboard_needs_a_session() {
    let app = TestApp::spawn_with_admin().await;
    let response = Browser::new(&app).get("/admin").await;

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/admin/login");
}

#[tokio::test]
async fn signing_in_opens_the_dashboard_until_signing_out() {
    let app = TestApp::spawn_with_admin().await;
    let browser = Browser::new(&app);

    let response = browser.sign_in(ADMIN_PASSWORD).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/admin");

    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.starts_with(&format!("{SESSION_COOKIE}=")));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert!(cookie.contains("Path=/admin"));
    assert!(
        !cookie.contains(ADMIN_USERNAME),
        "the session cookie is encrypted"
    );

    let dashboard = browser.get("/admin").await;
    assert_eq!(dashboard.status(), StatusCode::OK);
    let body = dashboard.text().await.unwrap();
    assert!(body.contains(&format!("Signed in as {ADMIN_USERNAME}")));
    assert!(body.contains(r#"action="/admin/logout""#));

    let login_again = browser.get("/admin/login").await;
    assert_eq!(location(&login_again), "/admin");

    let logout = browser.post("/admin/logout", &[]).await;
    assert_eq!(logout.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&logout), "/");

    let after = browser.get("/admin").await;
    assert_eq!(after.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&after), "/admin/login");
}

#[tokio::test]
async fn wrong_credentials_are_rejected_without_a_session() {
    let app = TestApp::spawn_with_admin().await;
    let browser = Browser::new(&app);

    let response = browser.sign_in("not the password").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("Wrong username or password.")
    );

    let unknown = browser
        .post(
            "/admin/login",
            &[("username", "root"), ("password", ADMIN_PASSWORD)],
        )
        .await;
    assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);

    assert_eq!(browser.get("/admin").await.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn cross_site_posts_are_refused() {
    let app = TestApp::spawn_with_admin().await;
    let browser = Browser::new(&app);

    let response = browser
        .post_from(
            "/admin/login",
            &[("username", ADMIN_USERNAME), ("password", ADMIN_PASSWORD)],
            "https://evil.example",
        )
        .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn repeated_failures_lock_the_client_out() {
    let app = TestApp::spawn_with_admin().await;
    let browser = Browser::new(&app);

    for _ in 0..MAX_ATTEMPTS_PER_CLIENT {
        assert_eq!(
            browser.sign_in("guess").await.status(),
            StatusCode::UNAUTHORIZED
        );
    }

    let locked = browser.sign_in(ADMIN_PASSWORD).await;
    assert_eq!(locked.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(
        locked
            .text()
            .await
            .unwrap()
            .contains("Too many failed attempts")
    );
    assert_eq!(browser.get("/admin").await.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn forged_session_cookies_are_ignored() {
    let app = TestApp::spawn_with_admin().await;

    let response = Client::builder()
        .redirect(Policy::none())
        .build()
        .unwrap()
        .get(format!("{}/admin", app.address))
        .header(header::COOKIE, format!("{SESSION_COOKIE}=forged"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/admin/login");
}

#[tokio::test]
async fn parallel_attempts_cannot_get_past_the_limit() {
    let app = TestApp::spawn_with_admin().await;
    let client = Client::new();

    let attempts: Vec<_> = (0..20)
        .map(|_| {
            let client = client.clone();
            let url = format!("{}/admin/login", app.address);
            let origin = app.address.clone();
            tokio::spawn(async move {
                client
                    .post(url)
                    .header(header::ORIGIN, origin)
                    .form(&[("username", ADMIN_USERNAME), ("password", "guess")])
                    .send()
                    .await
                    .unwrap()
                    .status()
            })
        })
        .collect();

    let mut statuses = Vec::new();
    for attempt in attempts {
        statuses.push(attempt.await.unwrap());
    }
    let checked = statuses
        .iter()
        .filter(|status| **status == StatusCode::UNAUTHORIZED)
        .count();
    let refused = statuses
        .iter()
        .filter(|status| **status == StatusCode::TOO_MANY_REQUESTS)
        .count();

    assert_eq!(checked, MAX_ATTEMPTS_PER_CLIENT as usize);
    assert_eq!(refused, 20 - checked);
}
