use actix_web::{HttpRequest, HttpResponse, http::header::ContentType, web::Html};
use chess_module::LichessState;
use games_module::SteamState;
use music_module::SpotifyState;
use search_engine::{PostsSearchEngine, utils::DEFAULT_LANG};
use std::collections::HashMap;
use std::sync::Arc;

use crate::i18n::{LANG_PARAM, Locale, lang_query};
use crate::site_config::SiteConfig;

pub mod chess_controller;
pub mod fallback_controller;
pub mod feed_controller;
pub mod graph_controller;
pub mod metadata_controller;
pub mod music_controller;
pub mod news_controller;
pub mod obsidian_markdown;
pub mod posts_controller;
pub mod routines_controller;
pub mod steam_controller;
pub mod tags_controller;

pub struct AppState {
    pub garden_path: String,
    pub shell: PageShell,
    pub lichess_state: LichessState,
    pub spotify_state: Arc<tokio::sync::Mutex<SpotifyState>>,
    pub steam_state: SteamState,
    pub post_search_engine: Arc<PostsSearchEngine>,
}

pub struct PageShell {
    app_name: String,
    templates: HashMap<Locale, String>,
}

impl PageShell {
    pub fn new(app_name: &str, template: &str, site: &SiteConfig) -> PageShell {
        let template = template.replace("{{THEME}}", &site.theme_style());

        PageShell {
            app_name: app_name.to_string(),
            templates: Locale::ALL
                .into_iter()
                .map(|locale| (locale, localize_template(&template, locale)))
                .collect(),
        }
    }

    pub fn from_template_dir(app_name: &str, site: &SiteConfig) -> PageShell {
        let template_path =
            std::env::var("TEMPLATE_PATH").unwrap_or_else(|_| "./templates".to_string());
        let template = std::fs::read_to_string(format!("{template_path}/index.html"))
            .expect("Failed to read index.html template");
        PageShell::new(app_name, &template, site)
    }

    pub fn app_name(&self) -> &str {
        &self.app_name
    }

    pub fn description(&self, locale: Locale) -> &str {
        locale.texts().site_description
    }

    pub fn wrap(&self, content: &str, locale: Locale) -> String {
        self.wrap_with_meta(content, &PageMeta::default(), None, locale)
    }

    pub fn wrap_with_meta(
        &self,
        content: &str,
        meta: &PageMeta,
        url: Option<&str>,
        locale: Locale,
    ) -> String {
        self.templates[&locale]
            .replace(
                "{{HEAD}}",
                &meta.head(&self.app_name, self.description(locale), url),
            )
            .replace("{{CONTENT}}", content)
    }
}

#[derive(Default)]
pub struct PageMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub is_article: bool,
    pub lang: Option<String>,
    pub translations: Vec<String>,
}

impl PageMeta {
    pub fn titled(title: impl Into<String>) -> PageMeta {
        PageMeta {
            title: Some(title.into()),
            ..PageMeta::default()
        }
    }

    pub fn full_title(&self, app_name: &str) -> String {
        match &self.title {
            Some(title) => format!("{title} · {app_name}"),
            None => app_name.to_string(),
        }
    }

    fn head(&self, app_name: &str, site_description: &str, url: Option<&str>) -> String {
        let description = self.description.as_deref().unwrap_or(site_description);
        let og_type = if self.is_article {
            "article"
        } else {
            "website"
        };
        let lang_url = |url: &str, lang: &str| format!("{url}?{LANG_PARAM}={lang}");
        let page_url = url.map(|url| match (&self.lang, self.translations.len() > 1) {
            (Some(lang), true) => lang_url(url, lang),
            _ => url.to_string(),
        });
        let feed_url = format!(
            "/rss.xml{}",
            lang_query(self.lang.as_deref().unwrap_or(DEFAULT_LANG))
        );

        maud::html! {
            title { (self.full_title(app_name)) }
            meta name="description" content=(description);
            meta property="og:site_name" content=(app_name);
            meta property="og:type" content=(og_type);
            meta property="og:title" content=(self.title.as_deref().unwrap_or(app_name));
            meta property="og:description" content=(description);
            @if let Some(page_url) = &page_url {
                meta property="og:url" content=(page_url);
                link rel="canonical" href=(page_url);
            }
            @if let (Some(url), true) = (url, self.translations.len() > 1) {
                @for lang in &self.translations {
                    link rel="alternate" hreflang=(lang) href=(lang_url(url, lang));
                }
                link rel="alternate" hreflang="x-default" href=(url);
            }
            meta name="twitter:card" content="summary";
            link rel="alternate" type="application/rss+xml" title=(app_name) href=(feed_url);
        }
        .into_string()
    }
}

pub fn base_url(req: &HttpRequest) -> String {
    let info = req.connection_info();
    format!("{}://{}", info.scheme(), info.host())
}

pub fn encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn localize_template(template: &str, locale: Locale) -> String {
    locale
        .texts()
        .entries()
        .into_iter()
        .fold(template.to_string(), |html, (key, text)| {
            html.replace(
                &format!("{{{{t:{key}}}}}"),
                &maud::html! { (text) }.into_string(),
            )
        })
        .replace("{{LANG}}", locale.as_str())
        .replace("{{LANGUAGE_SWITCHER}}", &language_switcher(locale))
}

fn language_switcher(current: Locale) -> String {
    maud::html! {
        nav aria-label=(current.texts().language_label) class="flex gap-2" {
            @for locale in Locale::ALL {
                a
                href=(format!("?{LANG_PARAM}={}", locale.as_str()))
                hx-boost="false"
                hreflang=(locale.as_str())
                lang=(locale.as_str())
                aria-current=[(locale == current).then_some("true")]
                class="uppercase hover:text-primary-color aria-[current=true]:text-primary-color"
                { (locale.as_str()) }
            }
        }
    }
    .into_string()
}

pub fn is_partial_request(req: &HttpRequest) -> bool {
    let headers = req.headers();
    let is_htmx_req = headers.contains_key("HX-Request");
    let is_history_restore = headers.contains_key("HX-History-Restore-Request");
    let is_full_swap = headers
        .get("HX-Request-Type")
        .is_some_and(|request_type| request_type == "full");

    is_htmx_req && !is_history_restore && !is_full_swap
}

pub fn render_page(app_state: &AppState, req: &HttpRequest, content: &str) -> Html {
    render_page_with_meta(app_state, req, content, &PageMeta::default())
}

pub fn render_page_with_meta(
    app_state: &AppState,
    req: &HttpRequest,
    content: &str,
    meta: &PageMeta,
) -> Html {
    if is_partial_request(req) {
        let title = maud::html! { title { (meta.full_title(app_state.shell.app_name())) } };
        Html::new(format!("{}{content}", title.into_string()))
    } else {
        let url = format!("{}{}", base_url(req), req.path());
        Html::new(
            app_state
                .shell
                .wrap_with_meta(content, meta, Some(&url), Locale::negotiate(req)),
        )
    }
}

pub fn render_not_found(app_state: &AppState, req: &HttpRequest) -> HttpResponse {
    let locale = Locale::negotiate(req);
    let t = locale.texts();
    let content = maud::html! {
        div class="flex flex-col w-full gap-4 p-6 lg:p-8" {
            h1 class="text-4xl text-primary-color" { "404" }
            p { (t.not_found_message) }
            a href="/posts/welcome" hx-target="#main-section" class="hover:text-primary-color underline" {
                (t.not_found_back)
            }
        }
    }
    .into_string();

    let body = if is_partial_request(req) {
        content
    } else {
        app_state.shell.wrap(&content, locale)
    };

    HttpResponse::NotFound()
        .content_type(ContentType::html())
        .body(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site_config::{DEFAULT_SITE_CONFIG_PATH, STATIC_DIR};
    use actix_web::test::TestRequest;

    fn site() -> SiteConfig {
        SiteConfig::load(DEFAULT_SITE_CONFIG_PATH, STATIC_DIR).unwrap()
    }

    fn shell(app_name: &str) -> PageShell {
        PageShell::from_template_dir(app_name, &site())
    }

    #[test]
    fn plain_browser_requests_need_the_full_page() {
        let req = TestRequest::get().to_http_request();
        assert!(!is_partial_request(&req));
    }

    #[test]
    fn htmx_requests_targeting_a_section_are_partial() {
        let req = TestRequest::get()
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-Request-Type", "partial"))
            .to_http_request();
        assert!(is_partial_request(&req));

        let req = TestRequest::get()
            .insert_header(("HX-Request", "true"))
            .to_http_request();
        assert!(is_partial_request(&req));
    }

    #[test]
    fn htmx_body_swaps_need_the_full_page() {
        let req = TestRequest::get()
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-Request-Type", "full"))
            .to_http_request();
        assert!(!is_partial_request(&req));
    }

    #[test]
    fn htmx_history_restores_need_the_full_page() {
        let req = TestRequest::get()
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-History-Restore-Request", "true"))
            .to_http_request();
        assert!(!is_partial_request(&req));
    }

    #[test]
    fn content_is_wrapped_into_the_index_template() {
        let page = shell("Test garden").wrap("<p id=\"marker\">hi</p>", Locale::En);

        assert!(page.starts_with("<!doctype html>"));
        assert!(page.contains("<title>Test garden</title>"));
        assert!(page.contains("<p id=\"marker\">hi</p>"));
        assert!(!page.contains("{{HEAD}}") && !page.contains("{{CONTENT}}"));
        assert!(page.contains(&format!(
            "<meta name=\"description\" content=\"{}\">",
            Locale::En.texts().site_description
        )));
        assert!(page.contains(r#"<meta property="og:type" content="website">"#));
        assert!(!page.contains("og:url"), "no URL without a request");
    }

    #[test]
    fn pages_apply_the_site_theme_after_the_compiled_css() {
        let page = shell("Garden").wrap("", Locale::En);

        let theme = site().theme_style();
        assert!(!page.contains("{{THEME}}"));
        assert!(page.contains(&theme));
        assert!(page.find("compiled.css").unwrap() < page.find(&theme).unwrap());
        assert!(page.contains(r#"<script src="/_static/theme.js"></script>"#));
    }

    #[test]
    fn page_meta_fills_titles_descriptions_and_links() {
        let meta = PageMeta {
            title: Some("Fog <notes>".to_string()),
            description: Some("About \"fog\"".to_string()),
            is_article: true,
            ..PageMeta::default()
        };

        let page =
            shell("Garden").wrap_with_meta("", &meta, Some("https://g.dev/posts/fog"), Locale::En);

        assert!(page.contains("<title>Fog &lt;notes&gt; · Garden</title>"));
        assert!(page.contains(r#"<meta property="og:title" content="Fog &lt;notes&gt;">"#));
        assert!(page.contains(r#"<meta name="description" content="About &quot;fog&quot;">"#));
        assert!(page.contains(r#"<meta property="og:type" content="article">"#));
        assert!(page.contains(r#"<meta property="og:url" content="https://g.dev/posts/fog">"#));
        assert!(page.contains(r#"<link rel="alternate" type="application/rss+xml""#));
    }

    #[test]
    fn translated_pages_link_every_language_version() {
        let meta = PageMeta {
            lang: Some("es".to_string()),
            translations: vec!["en".to_string(), "es".to_string()],
            ..PageMeta::titled("Niebla")
        };

        let page =
            shell("Garden").wrap_with_meta("", &meta, Some("https://g.dev/posts/fog"), Locale::En);

        assert!(page.contains(r#"<link rel="canonical" href="https://g.dev/posts/fog?lang=es">"#));
        assert!(
            page.contains(r#"<meta property="og:url" content="https://g.dev/posts/fog?lang=es">"#)
        );
        assert!(page.contains(
            r#"<link rel="alternate" hreflang="en" href="https://g.dev/posts/fog?lang=en">"#
        ));
        assert!(page.contains(
            r#"<link rel="alternate" hreflang="es" href="https://g.dev/posts/fog?lang=es">"#
        ));
        assert!(page.contains(
            r#"<link rel="alternate" hreflang="x-default" href="https://g.dev/posts/fog">"#
        ));
        assert!(page.contains(r#"href="/rss.xml?lang=es""#));
    }

    #[test]
    fn untranslated_pages_keep_a_plain_canonical_and_no_hreflang() {
        let meta = PageMeta {
            lang: Some("en".to_string()),
            translations: vec!["en".to_string()],
            ..PageMeta::titled("Fog")
        };

        let page =
            shell("Garden").wrap_with_meta("", &meta, Some("https://g.dev/posts/fog"), Locale::En);

        assert!(page.contains(r#"<link rel="canonical" href="https://g.dev/posts/fog">"#));
        assert!(!page.contains(r#"rel="alternate" hreflang"#));
        assert!(page.contains(r#"href="/rss.xml""#));
    }

    #[test]
    fn every_template_placeholder_is_filled_in_every_language() {
        let shell = shell("Garden");
        for locale in Locale::ALL {
            let html = shell.wrap("", locale);
            assert!(!html.contains("{{"), "{}", locale.as_str());
            assert!(html.contains(&format!(r#"<html lang="{}">"#, locale.as_str())));
        }
    }

    #[test]
    fn spanish_pages_use_spanish_texts() {
        let page = shell("Garden").wrap("", Locale::Es);

        assert!(page.contains(r#"<html lang="es">"#));
        assert!(page.contains("INICIO"));
        assert!(page.contains(r#"placeholder="Busca cualquier post...""#));
        assert!(page.contains(&format!(
            r#"<meta name="description" content="{}">"#,
            Locale::Es.texts().site_description
        )));
        assert!(!page.contains(">HOME<"));
    }

    #[test]
    fn template_texts_are_html_escaped() {
        let html = localize_template(
            r#"<p title="{{t:switcher_help}}">{{t:tag_back}}</p>"#,
            Locale::En,
        );

        assert_eq!(
            html,
            r#"<p title="↑↓ to move · Enter to open · Esc to close">← All tags</p>"#
        );
    }

    #[test]
    fn the_language_switcher_marks_the_current_language() {
        let switcher = language_switcher(Locale::Es);

        assert!(switcher.contains(r#"<nav aria-label="Idioma""#));
        assert!(
            switcher.contains(r#"href="?lang=en" hx-boost="false" hreflang="en" lang="en" class="#)
        );
        assert!(switcher.contains(
            r#"href="?lang=es" hx-boost="false" hreflang="es" lang="es" aria-current="true""#
        ));
    }

    #[test]
    fn paths_are_percent_encoded_except_slashes() {
        assert_eq!(encode_path("gaming/horror"), "gaming/horror");
        assert_eq!(encode_path("digital garden"), "digital%20garden");
        assert_eq!(encode_path("c#ñ"), "c%23%C3%B1");
    }
}
