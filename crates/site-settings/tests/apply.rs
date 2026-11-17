use std::path::{Path, PathBuf};

use serde_json::json;
use site_config::settings::{FontRequest, FontRequests, REQUEST_FENCE, SettingsRequest};
use site_config::{Features, FontFallback, Link, SiteConfig};
use site_settings::{Args, Fontsource, Removed, apply, render_config, run};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CURRENT: &str = r##"
[theme.colors]
background = "#1e242c"
bright = "#dbdfe5"
primary = "#f58a07"
secondary = "#3f826d"
accent = "#bc6c25"
shade = "#323c49"

[theme.fonts.body]
family = "Old Body"

[[theme.fonts.body.faces]]
src = "fonts/old-body/regular.woff2"
weight = 600

[theme.fonts.heading]
family = "Old Heading"

[[theme.fonts.heading.faces]]
src = "fonts/old-heading/regular.woff2"
weight = 600
"##;

const WOFF2: &[u8] = b"wOF2 pretend font bytes";

struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Repo {
        let repo = Repo {
            dir: tempfile::tempdir().unwrap(),
        };
        for folder in ["old-body", "old-heading"] {
            let folder = repo.static_dir().join("fonts").join(folder);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("regular.woff2"), WOFF2).unwrap();
        }
        std::fs::write(repo.config_path(), CURRENT).unwrap();
        repo
    }

    fn static_dir(&self) -> PathBuf {
        self.dir.path().join("static")
    }

    fn config_path(&self) -> PathBuf {
        self.dir.path().join("site.toml")
    }

    fn config(&self) -> SiteConfig {
        SiteConfig::load(self.config_path(), self.static_dir()).unwrap()
    }

    fn font(&self, src: &str) -> PathBuf {
        self.static_dir().join(src)
    }
}

async fn fontsource() -> (MockServer, Fontsource) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/fonts/inter"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "inter",
            "family": "Inter",
            "subsets": ["cyrillic", "latin", "latin-ext"],
            "weights": [400, 600],
            "styles": ["normal"],
            "defSubset": "latin",
            "type": "google",
            "unicodeRange": {
                "latin": "U+0000-00FF,U+0131",
                "latin-ext": "U+0100-02BA",
                "cyrillic": "U+0400-045F"
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/fonts/noto-sans-jp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "noto-sans-jp",
            "family": "Noto Sans JP",
            "subsets": ["japanese"],
            "weights": [400],
            "defSubset": "japanese",
            "type": "google"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/fonts/custom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "custom",
            "family": "Custom",
            "subsets": ["latin"],
            "weights": [400],
            "defSubset": "latin",
            "type": "commercial",
            "category": "serif"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/fonts/dseg7-classic"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "dseg7-classic",
            "family": "DSEG7 Classic",
            "subsets": ["latin"],
            "weights": [400],
            "defSubset": "latin",
            "type": "other",
            "category": "other"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/fonts/iosevka"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "iosevka",
            "family": "Iosevka",
            "subsets": ["latin"],
            "weights": [400, 600],
            "defSubset": "latin",
            "type": "other",
            "category": "monospace",
            "unicodeRange": {}
        })))
        .mount(&server)
        .await;
    for file in [
        "/cdn/inter@latest/latin-600-normal.woff2",
        "/cdn/inter@latest/latin-ext-600-normal.woff2",
        "/cdn/inter@latest/latin-400-normal.woff2",
        "/cdn/inter@latest/latin-ext-400-normal.woff2",
        "/cdn/noto-sans-jp@latest/japanese-400-normal.woff2",
    ] {
        Mock::given(method("GET"))
            .and(path(file))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(WOFF2))
            .mount(&server)
            .await;
    }
    let mut large_font = WOFF2.to_vec();
    large_font.resize(400 * 1024, 0);
    Mock::given(method("GET"))
        .and(path("/cdn/iosevka@latest/latin-600-normal.woff2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(large_font))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let fontsource = Fontsource::new(server.uri(), format!("{}/cdn", server.uri()));
    (server, fontsource)
}

fn request(repo: &Repo) -> SettingsRequest {
    SettingsRequest::keeping(&repo.config())
}

fn inter(weight: u16) -> FontRequest {
    FontRequest::Fontsource {
        id: "inter".to_string(),
        weight,
        fallback: FontFallback::SansSerif,
    }
}

fn shown(path: &Path, repo: &Repo) -> String {
    format!(
        "static/{}",
        path.strip_prefix(repo.static_dir()).unwrap().display()
    )
}

#[tokio::test]
async fn colors_and_sections_change_without_touching_fonts() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let mut request = request(&repo);
    request.colors.primary = "#e11d48".to_string();
    request.features.chess = false;

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        Some(7),
    )
    .await
    .unwrap();

    assert_eq!(outcome.config.theme.colors.primary, "#e11d48");
    assert!(!outcome.config.features.chess);
    assert_eq!(outcome.config.theme.fonts, repo.config().theme.fonts);
    assert!(outcome.added.is_empty() && outcome.removed.is_empty());
    assert!(outcome.summary.starts_with("Settings requested in #7."));
    assert!(
        outcome
            .summary
            .contains("| primary | `#f58a07` | `#e11d48` |")
    );
    assert!(!outcome.summary.contains("| background |"));
    assert!(outcome.summary.contains("- chess: on → off"));
    assert!(!outcome.summary.contains("**Fonts**"));
    assert!(outcome.summary.ends_with("Closes #7\n"));
    assert!(!outcome.summary.contains("**Site**"));
}

#[tokio::test]
async fn identity_changes_are_applied_and_summarized() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let mut request = request(&repo);
    request.identity.name = "Ana's garden".to_string();
    request.identity.source_url = Some("https://github.com/ana/garden".to_string());
    request
        .identity
        .description
        .insert("es".to_string(), "Notas de Ana".to_string());
    request.identity.links.push(Link {
        label: "MAIL".to_string(),
        url: "mailto:ana@example.com".to_string(),
    });

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap();

    assert_eq!(outcome.config.identity, request.identity);
    assert!(
        outcome
            .summary
            .contains("- name: My digital garden → Ana's garden")
    );
    assert!(
        outcome
            .summary
            .contains("- source link: none → https://github.com/ana/garden")
    );
    assert!(outcome.summary.contains("- description (es): Notas de Ana"));
    assert!(outcome.summary.contains("- footer links: MAIL"));

    let mut invalid = request.clone();
    invalid.identity.links[0].url = "javascript:alert(1)".to_string();
    let problems = apply(
        &invalid,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap_err();
    assert!(problems[0].starts_with("site.links[0].url"));
}

#[tokio::test]
async fn new_fonts_are_downloaded_and_unused_ones_removed() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let request = SettingsRequest {
        fonts: FontRequests {
            body: inter(600),
            heading: FontRequest::keep(),
        },
        ..request(&repo)
    };

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap();

    let body = &outcome.config.theme.fonts.body;
    assert_eq!(body.family, "Inter");
    let sources: Vec<&str> = body.faces.iter().map(|face| face.src.as_str()).collect();
    assert_eq!(
        sources,
        vec![
            "fonts/inter/latin-600-normal.woff2",
            "fonts/inter/latin-ext-600-normal.woff2"
        ]
    );
    assert_eq!(
        body.faces[0].unicode_range.as_deref(),
        Some("U+0000-00FF,U+0131")
    );
    assert_eq!(body.faces[1].unicode_range.as_deref(), Some("U+0100-02BA"));
    assert_eq!(std::fs::read(repo.font(sources[0])).unwrap(), WOFF2);

    assert_eq!(outcome.config.theme.fonts.heading.family, "Old Heading");
    assert!(!repo.static_dir().join("fonts/old-body").exists());
    assert!(repo.font("fonts/old-heading/regular.woff2").exists());
    assert_eq!(
        outcome.removed,
        vec![Removed {
            path: repo.static_dir().join("fonts/old-body"),
            is_folder: true,
        }]
    );

    let added: Vec<String> = outcome.added.iter().map(|p| shown(p, &repo)).collect();
    assert_eq!(
        added,
        vec![
            "static/fonts/inter/latin-600-normal.woff2",
            "static/fonts/inter/latin-ext-600-normal.woff2"
        ]
    );
    assert!(
        outcome
            .summary
            .contains("- body: Old Body → Inter (weight 600, sans-serif fallback)")
    );
    assert!(
        outcome
            .summary
            .contains("- added `static/fonts/inter/latin-600-normal.woff2` (1 KB)")
    );
    assert!(!outcome.summary.contains("Heads-up"));
    assert!(
        outcome
            .summary
            .contains("- removed `static/fonts/old-body/`")
    );
    assert!(!outcome.summary.contains("Closes"));
}

#[tokio::test]
async fn both_roles_can_share_a_font_and_default_subsets_are_used() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let request = SettingsRequest {
        fonts: FontRequests {
            body: FontRequest::Fontsource {
                id: "noto-sans-jp".to_string(),
                weight: 400,
                fallback: FontFallback::SansSerif,
            },
            heading: FontRequest::Fontsource {
                id: "noto-sans-jp".to_string(),
                weight: 400,
                fallback: FontFallback::Serif,
            },
        },
        ..request(&repo)
    };

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        outcome.config.theme.fonts.body.faces[0].src,
        "fonts/noto-sans-jp/japanese-400-normal.woff2"
    );
    assert_eq!(outcome.config.theme.fonts.body.faces[0].unicode_range, None);
    assert_eq!(outcome.added.len(), 1);
    assert_eq!(outcome.removed.len(), 2);
}

#[tokio::test]
async fn open_source_fonts_are_installed_and_large_files_flagged() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let request = SettingsRequest {
        fonts: FontRequests {
            body: FontRequest::Fontsource {
                id: "iosevka".to_string(),
                weight: 600,
                fallback: FontFallback::Monospace,
            },
            heading: FontRequest::keep(),
        },
        ..request(&repo)
    };

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap();

    let body = &outcome.config.theme.fonts.body;
    assert_eq!(body.family, "Iosevka");
    assert_eq!(body.faces.len(), 1);
    assert_eq!(body.faces[0].src, "fonts/iosevka/latin-600-normal.woff2");
    assert_eq!(body.faces[0].unicode_range, None);
    assert!(
        outcome
            .summary
            .contains("- added `static/fonts/iosevka/latin-600-normal.woff2` (400 KB)")
    );
    assert!(
        outcome
            .summary
            .contains("**Heads-up:** some font files are larger than 300 KB")
    );
}

#[tokio::test]
async fn unused_files_are_removed_from_folders_that_stay() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let inter_dir = repo.static_dir().join("fonts/inter");
    std::fs::create_dir_all(&inter_dir).unwrap();
    std::fs::write(inter_dir.join("hand-made-subset.woff2"), WOFF2).unwrap();
    std::fs::write(
        repo.static_dir().join("fonts/old-heading/stray.woff2"),
        WOFF2,
    )
    .unwrap();
    std::fs::write(repo.static_dir().join("fonts/README"), "not a folder").unwrap();
    let request = SettingsRequest {
        fonts: FontRequests {
            body: inter(600),
            heading: FontRequest::keep(),
        },
        ..request(&repo)
    };

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap();

    let removed: Vec<(String, bool)> = outcome
        .removed
        .iter()
        .map(|item| (shown(&item.path, &repo), item.is_folder))
        .collect();
    assert_eq!(
        removed,
        vec![
            (
                "static/fonts/inter/hand-made-subset.woff2".to_string(),
                false
            ),
            ("static/fonts/old-body".to_string(), true),
            ("static/fonts/old-heading/stray.woff2".to_string(), false),
        ]
    );
    assert!(repo.font("fonts/inter/latin-600-normal.woff2").exists());
    assert!(repo.font("fonts/old-heading/regular.woff2").exists());
    assert!(repo.static_dir().join("fonts/README").exists());
    assert!(
        outcome
            .summary
            .contains("- removed `static/fonts/inter/hand-made-subset.woff2`\n")
    );
    assert!(
        outcome
            .summary
            .contains("- removed `static/fonts/old-body/`\n")
    );
}

#[tokio::test]
async fn nested_faces_and_license_files_are_kept() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let nested = repo.static_dir().join("fonts/old-heading/v2");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("bold.woff2"), WOFF2).unwrap();
    std::fs::write(nested.join("unused.woff2"), WOFF2).unwrap();
    std::fs::write(
        repo.static_dir().join("fonts/old-heading/OFL.txt"),
        "license",
    )
    .unwrap();
    std::fs::write(repo.static_dir().join("fonts/old-body/LICENSE"), "license").unwrap();
    let raw = std::fs::read_to_string(repo.config_path())
        .unwrap()
        .replace(
            "fonts/old-heading/regular.woff2",
            "fonts/old-heading/v2/bold.woff2",
        );
    std::fs::write(repo.config_path(), raw).unwrap();
    let mut request = request(&repo);
    request.fonts.body = inter(600);

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap();

    let removed: Vec<(String, bool)> = outcome
        .removed
        .iter()
        .map(|item| (shown(&item.path, &repo), item.is_folder))
        .collect();
    assert_eq!(
        removed,
        vec![
            ("static/fonts/old-body".to_string(), true),
            ("static/fonts/old-heading/regular.woff2".to_string(), false),
            (
                "static/fonts/old-heading/v2/unused.woff2".to_string(),
                false
            ),
        ]
    );
    assert!(repo.font("fonts/old-heading/v2/bold.woff2").exists());
    assert!(repo.font("fonts/old-heading/OFL.txt").exists());
    assert!(
        outcome
            .config
            .missing_font_files(&repo.static_dir())
            .is_empty()
    );
}

#[tokio::test]
async fn kept_fonts_can_change_their_fallback() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let mut request = request(&repo);
    request.fonts.heading = FontRequest::Keep {
        fallback: Some(FontFallback::Monospace),
    };

    let outcome = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap();

    let heading = &outcome.config.theme.fonts.heading;
    assert_eq!(heading.family, "Old Heading");
    assert_eq!(heading.fallback, FontFallback::Monospace);
    assert_eq!(heading.faces, repo.config().theme.fonts.heading.faces);
    assert!(outcome.removed.is_empty());
    assert!(
        outcome
            .summary
            .contains("- heading: Old Heading → Old Heading (weight 600, monospace fallback)")
    );
}

#[tokio::test]
async fn unavailable_fonts_and_weights_are_refused() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;

    for (font, expected) in [
        (inter(900), "fonts.body: Inter has no weight 900"),
        (
            FontRequest::Fontsource {
                id: "custom".to_string(),
                weight: 400,
                fallback: FontFallback::Serif,
            },
            "is not a text font from Google Fonts or Fontsource's open-source fonts",
        ),
        (
            FontRequest::Fontsource {
                id: "dseg7-classic".to_string(),
                weight: 400,
                fallback: FontFallback::Serif,
            },
            "is not a text font",
        ),
        (
            FontRequest::Fontsource {
                id: "missing".to_string(),
                weight: 400,
                fallback: FontFallback::Serif,
            },
            "could not look up \"missing\"",
        ),
    ] {
        let request = SettingsRequest {
            fonts: FontRequests {
                body: font,
                heading: FontRequest::keep(),
            },
            ..request(&repo)
        };

        let problems = apply(
            &request,
            &repo.config(),
            &repo.static_dir(),
            &fontsource,
            None,
        )
        .await
        .unwrap_err();
        assert!(problems[0].contains(expected), "{problems:?}");
    }
    assert!(repo.static_dir().join("fonts/old-body").exists());
}

#[tokio::test]
async fn files_that_are_not_woff2_are_refused() {
    let repo = Repo::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/fonts/inter"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "inter",
            "family": "Inter",
            "subsets": ["latin"],
            "weights": [400],
            "defSubset": "latin",
            "type": "google"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>not a font</html>"))
        .mount(&server)
        .await;
    let fontsource = Fontsource::new(server.uri(), server.uri());
    let request = SettingsRequest {
        fonts: FontRequests {
            body: inter(400),
            heading: FontRequest::keep(),
        },
        ..request(&repo)
    };

    let problems = apply(
        &request,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap_err();

    assert!(problems[0].ends_with("is not a WOFF2 file"), "{problems:?}");
}

#[tokio::test]
async fn requests_that_change_nothing_or_are_invalid_are_refused() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;

    let unchanged = apply(
        &request(&repo),
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap_err();
    assert!(unchanged[0].contains("does not change anything"));

    let mut invalid = request(&repo);
    invalid.colors.shade = "grey".to_string();
    let problems = apply(
        &invalid,
        &repo.config(),
        &repo.static_dir(),
        &fontsource,
        None,
    )
    .await
    .unwrap_err();
    assert!(problems[0].starts_with("colors.shade"));
}

#[tokio::test]
async fn running_the_tool_rewrites_site_toml_and_the_summary() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let request = SettingsRequest {
        fonts: FontRequests {
            body: FontRequest::keep(),
            heading: inter(400),
        },
        features: Features {
            music: false,
            ..Features::default()
        },
        ..request(&repo)
    };
    let issue_body = format!(
        "Created from the admin page.\n\n{REQUEST_FENCE}\n{}```\n",
        request.to_toml()
    );
    let request_path = repo.dir.path().join("request.md");
    let summary_path = repo.dir.path().join("summary.md");
    std::fs::write(&request_path, issue_body).unwrap();

    let args = Args {
        request: request_path,
        summary: summary_path.clone(),
        issue: Some(12),
        config: repo.config_path(),
        static_dir: repo.static_dir(),
    };
    run(&args, &fontsource).await.unwrap();

    let written = repo.config();
    assert_eq!(written.theme.fonts.heading.family, "Inter");
    assert!(!written.features.music);
    assert!(
        std::fs::read_to_string(repo.config_path())
            .unwrap()
            .contains("[[theme.fonts.heading.faces]]")
    );
    let summary = std::fs::read_to_string(summary_path).unwrap();
    assert!(summary.contains("- heading: Old Heading → Inter (weight 400, sans-serif fallback)"));
    assert!(summary.contains("- music: on → off"));
    assert!(summary.contains("Closes #12"));
}

#[tokio::test]
async fn running_the_tool_reports_unreadable_inputs() {
    let repo = Repo::new();
    let (_server, fontsource) = fontsource().await;
    let args = Args {
        request: repo.dir.path().join("missing.md"),
        summary: repo.dir.path().join("summary.md"),
        issue: None,
        config: repo.config_path(),
        static_dir: repo.static_dir(),
    };

    let problems = run(&args, &fontsource).await.unwrap_err();
    assert!(problems[0].starts_with("could not read"));

    std::fs::write(&args.request, "no request block").unwrap();
    let problems = run(&args, &fontsource).await.unwrap_err();
    assert!(problems[0].contains("site-settings block"));

    std::fs::write(
        &args.request,
        format!("{REQUEST_FENCE}\n{}```\n", request(&repo).to_toml()),
    )
    .unwrap();
    std::fs::write(repo.config_path(), "broken").unwrap();
    let problems = run(&args, &fontsource).await.unwrap_err();
    assert!(problems[0].contains("is invalid"));
}

#[test]
fn rendered_configs_load_back_unchanged() {
    let repo = Repo::new();
    let config = repo.config();

    let rendered = render_config(&config);

    assert_eq!(SiteConfig::parse(&rendered).unwrap(), config);
    assert!(rendered.contains("[theme.colors]"));
    assert!(rendered.contains("[[theme.fonts.body.faces]]"));
}

#[test]
fn arguments_are_parsed_with_defaults() {
    let parse = |args: &[&str]| Args::parse(args.iter().map(|arg| arg.to_string()));

    let args = parse(&["--request", "r.md", "--summary", "s.md"]).unwrap();
    assert_eq!(args.config, PathBuf::from("site.toml"));
    assert_eq!(args.static_dir, PathBuf::from("static"));
    assert_eq!(args.issue, None);

    let args = parse(&[
        "--request",
        "r.md",
        "--summary",
        "s.md",
        "--issue",
        "42",
        "--config",
        "c.toml",
        "--static-dir",
        "assets",
    ])
    .unwrap();
    assert_eq!(args.issue, Some(42));
    assert_eq!(args.config, PathBuf::from("c.toml"));
    assert_eq!(args.static_dir, PathBuf::from("assets"));

    assert!(
        parse(&["--summary", "s.md"])
            .unwrap_err()
            .contains("--request")
    );
    assert!(
        parse(&["--request", "r.md"])
            .unwrap_err()
            .contains("--summary")
    );
    assert!(parse(&["--request"]).unwrap_err().contains("needs a value"));
    assert!(
        parse(&["--issue", "x"])
            .unwrap_err()
            .contains("must be a number")
    );
    assert!(
        parse(&["--verbose"])
            .unwrap_err()
            .contains("unknown argument")
    );
}
