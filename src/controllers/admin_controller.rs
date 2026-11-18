use std::time::{Duration as StdDuration, Instant, SystemTime, UNIX_EPOCH};

use actix_session::{
    Session, SessionExt, SessionMiddleware,
    config::{CookieContentSecurity, PersistentSession},
    storage::CookieSessionStore,
};
use actix_web::{
    FromRequest, HttpRequest, HttpResponse, Responder, ResponseError,
    cookie::{Key, SameSite, time::Duration},
    dev::Payload,
    get,
    http::{StatusCode, header},
    middleware::DefaultHeaders,
    post, web,
};
use admin_module::{AdminStore, Admission, FontCatalog, LoginThrottle, MAX_CREDENTIAL_BYTES};
use maud::html;
use serde::Deserialize;

use crate::controllers::admin_settings::{self, FormValues, SettingsForm};
use crate::controllers::{AppState, render_page};
use crate::i18n::{Locale, Texts};
use crate::settings::GithubRepo;

pub const SESSION_COOKIE: &str = "guslee_admin";
pub const SESSION_TTL_SECS: i64 = 4 * 60 * 60;
pub const MAX_ATTEMPTS_PER_CLIENT: u32 = 5;
pub const MAX_ATTEMPTS_OVERALL: u32 = 50;
pub const OVERALL_SLOWDOWN: StdDuration = StdDuration::from_secs(3);
pub const ATTEMPT_WINDOW: StdDuration = StdDuration::from_secs(15 * 60);
const SESSION_USER: &str = "admin";
const SESSION_ISSUED_AT: &str = "issued_at";
const CLIENT_IP_HEADER: &str = "Fly-Client-IP";

pub struct AdminAuth {
    store: AdminStore,
    throttle: LoginThrottle,
    secure_cookies: bool,
    trust_fly_client_ip: bool,
    pub(crate) fonts: FontCatalog,
    pub(crate) repository: Option<GithubRepo>,
}

impl AdminAuth {
    pub fn new(store: AdminStore, fonts: FontCatalog, repository: Option<GithubRepo>) -> AdminAuth {
        AdminAuth {
            store,
            throttle: LoginThrottle::new(
                MAX_ATTEMPTS_PER_CLIENT,
                MAX_ATTEMPTS_OVERALL,
                OVERALL_SLOWDOWN,
                ATTEMPT_WINDOW,
            ),
            secure_cookies: true,
            trust_fly_client_ip: false,
            fonts,
            repository,
        }
    }

    pub fn trusting_fly_client_ip(self, trust: bool) -> AdminAuth {
        AdminAuth {
            trust_fly_client_ip: trust,
            ..self
        }
    }

    pub fn with_insecure_cookies(self) -> AdminAuth {
        AdminAuth {
            secure_cookies: false,
            ..self
        }
    }

    fn cookie_key(&self) -> Key {
        Key::derive_from(self.store.session_key_material())
    }
}

pub fn configure_services(cfg: &mut web::ServiceConfig, admin: web::Data<AdminAuth>) {
    let sessions = SessionMiddleware::builder(CookieSessionStore::default(), admin.cookie_key())
        .cookie_name(SESSION_COOKIE.to_string())
        .cookie_path("/admin".to_string())
        .cookie_secure(admin.secure_cookies)
        .cookie_http_only(true)
        .cookie_same_site(SameSite::Strict)
        .cookie_content_security(CookieContentSecurity::Private)
        .session_lifecycle(
            PersistentSession::default().session_ttl(Duration::seconds(SESSION_TTL_SECS)),
        )
        .build();

    let security_headers = DefaultHeaders::new()
        .add((header::CACHE_CONTROL, "no-store"))
        .add((header::X_FRAME_OPTIONS, "DENY"))
        .add((header::REFERRER_POLICY, "same-origin"));

    cfg.service(
        web::scope("/admin")
            .app_data(admin)
            .wrap(sessions)
            .wrap(security_headers)
            .service(dashboard)
            .service(submit_settings)
            .service(login_page)
            .service(login)
            .service(logout),
    );
}

pub struct SignedInAdmin {
    username: String,
}

#[derive(Debug)]
pub struct SignInRequired;

impl std::fmt::Display for SignInRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an admin session is required")
    }
}

impl ResponseError for SignInRequired {
    fn status_code(&self) -> StatusCode {
        StatusCode::SEE_OTHER
    }

    fn error_response(&self) -> HttpResponse {
        redirect("/admin/login")
    }
}

impl FromRequest for SignedInAdmin {
    type Error = SignInRequired;
    type Future = std::future::Ready<Result<SignedInAdmin, SignInRequired>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        std::future::ready(
            signed_in_admin(&req.get_session(), unix_now())
                .map(|username| SignedInAdmin { username })
                .ok_or(SignInRequired),
        )
    }
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

#[get("")]
async fn dashboard(
    app_state: web::Data<AppState>,
    admin: web::Data<AdminAuth>,
    signed_in: SignedInAdmin,
    req: HttpRequest,
    locale: Locale,
) -> HttpResponse {
    let values = FormValues::from_site(&app_state.site);
    let content = admin_settings::render(
        &app_state,
        &admin,
        &signed_in.username,
        &values,
        &[],
        locale.texts(),
    )
    .await;
    page(&app_state, &req, StatusCode::OK, &content)
}

#[post("/settings")]
async fn submit_settings(
    app_state: web::Data<AppState>,
    admin: web::Data<AdminAuth>,
    signed_in: SignedInAdmin,
    req: HttpRequest,
    locale: Locale,
    form: web::Form<SettingsForm>,
) -> HttpResponse {
    let t = locale.texts();
    if !is_same_origin(&req) {
        return page(
            &app_state,
            &req,
            StatusCode::FORBIDDEN,
            &message(t.admin_forbidden),
        );
    }
    let username = signed_in.username;

    let values = form.into_inner().into_values(&app_state.site);
    match admin_settings::issue_url(&app_state.site, &admin, &values, t).await {
        Ok(url) => {
            log::info!("Admin {username} opened a site settings request");
            redirect(&url)
        }
        Err(problems) => {
            let content =
                admin_settings::render(&app_state, &admin, &username, &values, &problems, t).await;
            page(&app_state, &req, StatusCode::UNPROCESSABLE_ENTITY, &content)
        }
    }
}

#[get("/login")]
async fn login_page(
    app_state: web::Data<AppState>,
    session: Session,
    req: HttpRequest,
    locale: Locale,
) -> HttpResponse {
    if signed_in_admin(&session, unix_now()).is_some() {
        return redirect("/admin");
    }

    page(
        &app_state,
        &req,
        StatusCode::OK,
        &login_content(locale.texts(), None),
    )
}

#[post("/login")]
async fn login(
    app_state: web::Data<AppState>,
    admin: web::Data<AdminAuth>,
    session: Session,
    req: HttpRequest,
    locale: Locale,
    form: web::Form<LoginForm>,
) -> HttpResponse {
    let t = locale.texts();
    if !is_same_origin(&req) {
        return page(
            &app_state,
            &req,
            StatusCode::FORBIDDEN,
            &message(t.admin_forbidden),
        );
    }

    let client = client_id(&req, admin.trust_fly_client_ip);
    match admin.throttle.admit(&client, Instant::now()) {
        Admission::Blocked => {
            log::warn!("Blocked admin sign-in attempt from {client}");
            return page(
                &app_state,
                &req,
                StatusCode::TOO_MANY_REQUESTS,
                &login_content(t, Some(t.admin_login_throttled)),
            );
        }
        Admission::Allowed { delay } if !delay.is_zero() => {
            log::warn!("Many failed admin sign-ins overall, slowing down {client}");
            tokio::time::sleep(delay).await;
        }
        Admission::Allowed { .. } => {}
    }

    let LoginForm { username, password } = form.into_inner();
    if !admin.store.verify(&username, &password).await {
        log::warn!("Failed admin sign-in attempt from {client}");
        return page(
            &app_state,
            &req,
            StatusCode::UNAUTHORIZED,
            &login_content(t, Some(t.admin_login_failed)),
        );
    }

    admin.throttle.record_success(&client);
    session.renew();
    let stored = session
        .insert(SESSION_USER, &username)
        .and_then(|_| session.insert(SESSION_ISSUED_AT, unix_now()));
    if stored.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    log::info!("Admin signed in from {client}");
    redirect("/admin")
}

#[post("/logout")]
async fn logout(session: Session, req: HttpRequest) -> HttpResponse {
    if !is_same_origin(&req) {
        return HttpResponse::Forbidden().finish();
    }

    session.purge();
    redirect("/")
}

fn login_content(t: &Texts, error: Option<&str>) -> String {
    let input_class = "rounded-box border border-shade-color bg-background-color px-2 py-1 outline-none focus:border-primary-color";

    html! {
        div class="flex flex-col w-full max-w-sm gap-6 p-6 lg:p-8" {
            h1 class="text-4xl text-primary-color" { (t.admin_login_title) }
            @if let Some(error) = error {
                p role="alert" class="text-primary-color" { (error) }
            }
            form method="post" action="/admin/login" hx-boost="false" class="flex flex-col gap-4" {
                label class="flex flex-col gap-1" {
                    span { (t.admin_username) }
                    input
                    type="text"
                    name="username"
                    autocomplete="username"
                    required
                    maxlength=(MAX_CREDENTIAL_BYTES)
                    class=(input_class);
                }
                label class="flex flex-col gap-1" {
                    span { (t.admin_password) }
                    input
                    type="password"
                    name="password"
                    autocomplete="current-password"
                    required
                    maxlength=(MAX_CREDENTIAL_BYTES)
                    class=(input_class);
                }
                button
                type="submit"
                class="rounded-box border border-primary-color px-3 py-1 cursor-pointer hover:bg-primary-color hover:text-background-color"
                { (t.admin_login_submit) }
            }
        }
    }
    .into_string()
}

pub(crate) fn logout_form(t: &Texts) -> maud::Markup {
    html! {
        form method="post" action="/admin/logout" hx-boost="false" {
            button
            type="submit"
            class="rounded-box border border-primary-color px-3 py-1 cursor-pointer hover:bg-primary-color hover:text-background-color"
            { (t.admin_logout) }
        }
    }
}

fn message(text: &str) -> String {
    html! {
        div class="flex flex-col w-full gap-4 p-6 lg:p-8" {
            p role="alert" class="text-primary-color" { (text) }
        }
    }
    .into_string()
}

fn page(
    app_state: &AppState,
    req: &HttpRequest,
    status: StatusCode,
    content: &str,
) -> HttpResponse {
    let mut response = render_page(app_state, req, content).respond_to(req);
    *response.status_mut() = status;
    response.map_into_boxed_body()
}

fn redirect(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, location))
        .finish()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

fn signed_in_admin(session: &Session, now: i64) -> Option<String> {
    let username = session.get::<String>(SESSION_USER).ok().flatten()?;
    let issued_at = session.get::<i64>(SESSION_ISSUED_AT).ok().flatten();

    match issued_at {
        Some(issued_at) if is_fresh(issued_at, now) => Some(username),
        _ => {
            session.purge();
            None
        }
    }
}

fn is_fresh(issued_at: i64, now: i64) -> bool {
    (0..SESSION_TTL_SECS).contains(&(now - issued_at))
}

fn is_same_origin(req: &HttpRequest) -> bool {
    let headers = req.headers();
    if headers
        .get("Sec-Fetch-Site")
        .is_some_and(|site| site != "same-origin" && site != "none")
    {
        return false;
    }

    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let host = req.connection_info().host().to_string();
    origin
        .to_str()
        .ok()
        .and_then(|origin| origin.split_once("://"))
        .is_some_and(|(_, origin_host)| origin_host == host)
}

fn client_id(req: &HttpRequest, trust_fly_client_ip: bool) -> String {
    req.headers()
        .get(CLIENT_IP_HEADER)
        .filter(|_| trust_fly_client_ip)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .or_else(|| req.peer_addr().map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn requests_without_an_origin_are_allowed() {
        let req = TestRequest::post()
            .insert_header((header::HOST, "garden.dev"))
            .to_http_request();
        assert!(is_same_origin(&req));
    }

    #[test]
    fn requests_from_the_same_host_are_allowed() {
        let req = TestRequest::post()
            .insert_header((header::HOST, "garden.dev"))
            .insert_header((header::ORIGIN, "https://garden.dev"))
            .insert_header(("Sec-Fetch-Site", "same-origin"))
            .to_http_request();
        assert!(is_same_origin(&req));
    }

    #[test]
    fn cross_site_requests_are_refused() {
        for (origin, fetch_site) in [
            (Some("https://evil.example"), None),
            (Some("https://garden.dev.evil.example"), None),
            (Some("null"), None),
            (None, Some("cross-site")),
            (None, Some("same-site")),
        ] {
            let mut req = TestRequest::post().insert_header((header::HOST, "garden.dev"));
            if let Some(origin) = origin {
                req = req.insert_header((header::ORIGIN, origin));
            }
            if let Some(fetch_site) = fetch_site {
                req = req.insert_header(("Sec-Fetch-Site", fetch_site));
            }
            assert!(
                !is_same_origin(&req.to_http_request()),
                "{origin:?} {fetch_site:?}"
            );
        }
    }

    #[test]
    fn the_fly_client_ip_is_only_trusted_on_fly() {
        let req = TestRequest::post()
            .peer_addr("10.0.0.1:1234".parse().unwrap())
            .insert_header((CLIENT_IP_HEADER, "203.0.113.7"))
            .to_http_request();
        assert_eq!(client_id(&req, true), "203.0.113.7");
        assert_eq!(client_id(&req, false), "10.0.0.1");

        let req = TestRequest::post()
            .peer_addr("10.0.0.1:1234".parse().unwrap())
            .to_http_request();
        assert_eq!(client_id(&req, true), "10.0.0.1");

        assert_eq!(
            client_id(&TestRequest::post().to_http_request(), true),
            "unknown"
        );
    }

    #[test]
    fn sessions_expire_after_their_ttl() {
        assert!(is_fresh(1_000, 1_000));
        assert!(is_fresh(1_000, 1_000 + SESSION_TTL_SECS - 1));
        assert!(!is_fresh(1_000, 1_000 + SESSION_TTL_SECS));
        assert!(!is_fresh(1_000, 999));
    }

    #[test]
    fn the_login_form_never_prefills_and_escapes_errors() {
        let t = Locale::En.texts();
        let form = login_content(t, Some("<b>nope</b>"));

        assert!(form.contains(r#"action="/admin/login""#));
        assert!(form.contains(r#"autocomplete="current-password""#));
        assert!(!form.contains("value="));
        assert!(form.contains("&lt;b&gt;nope&lt;/b&gt;"));
        assert!(!login_content(t, None).contains("role=\"alert\""));
    }
}
