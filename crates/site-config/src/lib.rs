use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub mod settings;

pub const DEFAULT_SITE_CONFIG_PATH: &str = "./site.toml";
pub const STATIC_DIR: &str = "./static";
const STATIC_URL_PREFIX: &str = "/_static";
const FONTS_DIR: &str = "fonts";
const MAX_FAMILY_LEN: usize = 64;
const DEFAULT_SITE_NAME: &str = "My digital garden";
const MAX_NAME_LEN: usize = 80;
const MAX_DESCRIPTION_LEN: usize = 300;
const MAX_LABEL_LEN: usize = 30;
const MAX_URL_LEN: usize = 300;
pub const MAX_LINKS: usize = 8;
const URL_SCHEMES: [&str; 3] = ["https://", "http://", "mailto:"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    #[serde(rename = "site", default)]
    pub identity: Identity,
    pub theme: Theme,
    #[serde(default)]
    pub features: Features,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    #[serde(default = "default_site_name")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub description: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub label: String,
    pub url: String,
}

fn default_site_name() -> String {
    DEFAULT_SITE_NAME.to_string()
}

impl Default for Identity {
    fn default() -> Self {
        Identity {
            name: default_site_name(),
            source_url: None,
            description: BTreeMap::new(),
            links: Vec::new(),
        }
    }
}

impl Identity {
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();

        if !is_plain_text(&self.name, MAX_NAME_LEN) {
            problems.push(format!(
                "site.name must be 1-{MAX_NAME_LEN} characters of plain text, got {:?}",
                self.name
            ));
        }
        if let Some(url) = &self.source_url
            && !is_link_url(url)
        {
            problems.push(format!(
                "site.source_url must be an http(s) URL without spaces or quotes, got {url:?}"
            ));
        }
        for (lang, text) in &self.description {
            if !is_language_code(lang) {
                problems.push(format!(
                    "site.description keys must be language codes like \"en\", got {lang:?}"
                ));
            }
            if !is_plain_text(text, MAX_DESCRIPTION_LEN) {
                problems.push(format!(
                    "site.description.{lang} must be 1-{MAX_DESCRIPTION_LEN} characters of plain text"
                ));
            }
        }
        if self.links.len() > MAX_LINKS {
            problems.push(format!("site.links can have at most {MAX_LINKS} links"));
        }
        for (index, link) in self.links.iter().enumerate() {
            if !is_plain_text(&link.label, MAX_LABEL_LEN) {
                problems.push(format!(
                    "site.links[{index}].label must be 1-{MAX_LABEL_LEN} characters of plain text, got {:?}",
                    link.label
                ));
            }
            if !is_link_url(&link.url) {
                problems.push(format!(
                    "site.links[{index}].url must be an http(s) or mailto URL without spaces or quotes, got {:?}",
                    link.url
                ));
            }
        }

        problems
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Features {
    pub news: bool,
    pub chess: bool,
    pub music: bool,
    pub games: bool,
    pub schedule: bool,
}

impl Features {
    pub fn entries(&self) -> [(&'static str, bool); 5] {
        [
            ("news", self.news),
            ("chess", self.chess),
            ("music", self.music),
            ("games", self.games),
            ("schedule", self.schedule),
        ]
    }
}

impl Default for Features {
    fn default() -> Self {
        Features {
            news: true,
            chess: true,
            music: true,
            games: true,
            schedule: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub colors: Colors,
    pub fonts: Fonts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Colors {
    pub background: String,
    pub bright: String,
    pub primary: String,
    pub secondary: String,
    pub accent: String,
    pub shade: String,
}

impl Colors {
    pub fn entries(&self) -> [(&'static str, &str); 6] {
        [
            ("background", &self.background),
            ("bright", &self.bright),
            ("primary", &self.primary),
            ("secondary", &self.secondary),
            ("accent", &self.accent),
            ("shade", &self.shade),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    pub body: Font,
    pub heading: Font,
}

impl Fonts {
    pub fn entries(&self) -> [(&'static str, &Font); 2] {
        [("body", &self.body), ("heading", &self.heading)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Font {
    pub family: String,
    #[serde(default)]
    pub fallback: FontFallback,
    pub faces: Vec<FontFace>,
}

impl Font {
    pub fn main_weight(&self) -> u16 {
        self.faces
            .iter()
            .find(|face| face.style == FontStyle::Normal)
            .or(self.faces.first())
            .map(|face| face.weight)
            .unwrap_or_else(default_weight)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FontFallback {
    Serif,
    #[default]
    SansSerif,
    Monospace,
    Cursive,
    SystemUi,
}

impl FontFallback {
    pub const ALL: [FontFallback; 5] = [
        FontFallback::SansSerif,
        FontFallback::Serif,
        FontFallback::Monospace,
        FontFallback::Cursive,
        FontFallback::SystemUi,
    ];

    pub fn as_css(self) -> &'static str {
        match self {
            FontFallback::Serif => "serif",
            FontFallback::SansSerif => "sans-serif",
            FontFallback::Monospace => "monospace",
            FontFallback::Cursive => "cursive",
            FontFallback::SystemUi => "system-ui",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontFace {
    pub src: String,
    #[serde(default = "default_weight")]
    pub weight: u16,
    #[serde(default)]
    pub style: FontStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unicode_range: Option<String>,
}

fn default_weight() -> u16 {
    400
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

impl FontStyle {
    fn as_css(self) -> &'static str {
        match self {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
        }
    }
}

#[derive(Debug)]
pub enum SiteConfigError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    Invalid(Vec<String>),
}

impl fmt::Display for SiteConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SiteConfigError::Read(e) => write!(f, "could not read file: {e}"),
            SiteConfigError::Parse(e) => write!(f, "could not parse TOML: {e}"),
            SiteConfigError::Invalid(problems) => write!(f, "{}", problems.join("; ")),
        }
    }
}

impl std::error::Error for SiteConfigError {}

impl SiteConfig {
    pub fn from_env() -> Self {
        let path = std::env::var("SITE_CONFIG_PATH")
            .unwrap_or_else(|_| DEFAULT_SITE_CONFIG_PATH.to_string());
        SiteConfig::load(&path, STATIC_DIR)
            .unwrap_or_else(|e| panic!("Invalid site config at {path}: {e}"))
    }

    pub fn load(
        path: impl AsRef<Path>,
        static_dir: impl AsRef<Path>,
    ) -> Result<Self, SiteConfigError> {
        let raw = std::fs::read_to_string(path).map_err(SiteConfigError::Read)?;
        let config = SiteConfig::parse(&raw)?;
        let missing = config.missing_font_files(static_dir.as_ref());
        if missing.is_empty() {
            Ok(config)
        } else {
            Err(SiteConfigError::Invalid(
                missing
                    .into_iter()
                    .map(|path| format!("font file {} does not exist", path.display()))
                    .collect(),
            ))
        }
    }

    pub fn parse(raw: &str) -> Result<Self, SiteConfigError> {
        let config: SiteConfig = toml::from_str(raw).map_err(SiteConfigError::Parse)?;
        let problems = config.validate();
        if problems.is_empty() {
            Ok(config)
        } else {
            Err(SiteConfigError::Invalid(problems))
        }
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = self.identity.validate();

        for (name, value) in self.theme.colors.entries() {
            if !is_hex_color(value) {
                problems.push(format!(
                    "theme.colors.{name} must be a #rrggbb color, got {value:?}"
                ));
            }
        }

        for (role, font) in self.theme.fonts.entries() {
            if !is_font_family(&font.family) {
                problems.push(format!(
                    "theme.fonts.{role}.family must be 1-{MAX_FAMILY_LEN} letters, digits, spaces, '-' or '_', got {:?}",
                    font.family
                ));
            }
            if font.faces.is_empty() {
                problems.push(format!("theme.fonts.{role}.faces must not be empty"));
            }
            for (index, face) in font.faces.iter().enumerate() {
                let at = format!("theme.fonts.{role}.faces[{index}]");
                if !is_font_src(&face.src) {
                    problems.push(format!(
                        "{at}.src must be a .woff2 path inside {FONTS_DIR}/, got {:?}",
                        face.src
                    ));
                }
                if !(100..=900).contains(&face.weight) {
                    problems.push(format!(
                        "{at}.weight must be between 100 and 900, got {}",
                        face.weight
                    ));
                }
                if let Some(range) = &face.unicode_range
                    && !is_unicode_range(range)
                {
                    problems.push(format!(
                        "{at}.unicode_range must look like \"U+0000-00FF, U+0131\", got {range:?}"
                    ));
                }
            }
        }

        problems
    }

    pub fn missing_font_files(&self, static_dir: &Path) -> Vec<PathBuf> {
        self.theme
            .fonts
            .entries()
            .into_iter()
            .flat_map(|(_, font)| &font.faces)
            .map(|face| static_dir.join(&face.src))
            .filter(|path| !path.is_file())
            .collect()
    }

    pub fn theme_style(&self) -> String {
        let mut css = String::new();

        for (_, font) in self.theme.fonts.entries() {
            for face in &font.faces {
                css.push_str(&format!(
                    "@font-face{{font-family:\"{}\";src:url(\"{STATIC_URL_PREFIX}/{}\") format(\"woff2\");font-weight:{};font-style:{};font-display:swap",
                    font.family,
                    face.src,
                    face.weight,
                    face.style.as_css()
                ));
                if let Some(range) = &face.unicode_range {
                    css.push_str(&format!(";unicode-range:{range}"));
                }
                css.push('}');
            }
        }

        css.push_str(":root{");
        for (name, value) in self.theme.colors.entries() {
            css.push_str(&format!("--color-{name}-color:{value};"));
        }
        for (role, font) in self.theme.fonts.entries() {
            css.push_str(&format!(
                "--font-{role}:\"{}\",{};--font-{role}-weight:{};",
                font.family,
                font.fallback.as_css(),
                font.main_weight()
            ));
        }
        css.push('}');

        format!("<style id=\"site-theme\">{css}</style>")
    }
}

fn is_plain_text(value: &str, max_chars: usize) -> bool {
    let count = value.chars().count();
    !value.trim().is_empty()
        && count <= max_chars
        && !value.chars().any(char::is_control)
        && !value.contains("{{")
}

fn is_link_url(value: &str) -> bool {
    URL_SCHEMES
        .iter()
        .any(|scheme| value.len() > scheme.len() && value.starts_with(scheme))
        && value.len() <= MAX_URL_LEN
        && !value.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '`')
        })
}

fn is_language_code(value: &str) -> bool {
    (2..=5).contains(&value.len()) && value.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}

pub fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn is_font_family(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed == value
        && value.len() <= MAX_FAMILY_LEN
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
}

fn is_font_src(value: &str) -> bool {
    let mut segments = value.split('/');
    segments.next() == Some(FONTS_DIR)
        && value.ends_with(".woff2")
        && segments.all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        })
}

fn is_unicode_range(value: &str) -> bool {
    value.split(',').all(|part| {
        let part = part.trim();
        let Some(codes) = part.strip_prefix("U+").or_else(|| part.strip_prefix("u+")) else {
            return false;
        };
        let mut bounds = codes.splitn(2, '-');
        let is_code = |code: &str, wildcard: bool| {
            (1..=6).contains(&code.len())
                && code
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() || (wildcard && c == '?'))
        };
        match (bounds.next(), bounds.next()) {
            (Some(start), None) => is_code(start, true),
            (Some(start), Some(end)) => is_code(start, false) && is_code(end, false),
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO_SITE_CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../site.toml");
    const REPO_STATIC_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../static");

    const VALID: &str = r##"
[theme.colors]
background = "#1e242c"
bright = "#dbdfe5"
primary = "#f58a07"
secondary = "#3f826d"
accent = "#bc6c25"
shade = "#323c49"

[theme.fonts.body]
family = "Iosevka"

[[theme.fonts.body.faces]]
src = "fonts/iosevka/regular.woff2"
weight = 600

[[theme.fonts.body.faces]]
src = "fonts/iosevka/italic.woff2"
style = "italic"
unicode_range = "U+0000-00FF, U+0131, U+02??"

[theme.fonts.heading]
family = "Tusker Grotesk"
fallback = "serif"

[[theme.fonts.heading.faces]]
src = "fonts/tusker-grotesk/regular.woff2"
"##;

    fn valid() -> SiteConfig {
        SiteConfig::parse(VALID).unwrap()
    }

    fn problems_after(change: impl FnOnce(&mut SiteConfig)) -> Vec<String> {
        let mut config = valid();
        change(&mut config);
        config.validate()
    }

    #[test]
    fn parses_a_valid_config_with_defaults() {
        let config = valid();

        let body = &config.theme.fonts.body;
        assert_eq!(body.fallback, FontFallback::SansSerif);
        assert_eq!(body.faces[0].weight, 600);
        assert_eq!(body.faces[0].style, FontStyle::Normal);
        assert_eq!(body.faces[1].weight, 400);
        assert_eq!(body.faces[1].style, FontStyle::Italic);
        assert_eq!(config.theme.fonts.heading.fallback, FontFallback::Serif);
    }

    #[test]
    fn the_committed_site_config_is_valid_and_its_fonts_exist() {
        let config = SiteConfig::load(REPO_SITE_CONFIG, REPO_STATIC_DIR);
        assert!(config.is_ok(), "{}", config.unwrap_err());
    }

    #[test]
    fn features_default_to_enabled_and_can_be_switched_off() {
        assert_eq!(valid().features, Features::default());
        assert!(Features::default().chess && Features::default().schedule);

        let raw = format!("{VALID}\n[features]\nchess = false\nschedule = false\n");
        let features = SiteConfig::parse(&raw).unwrap().features;
        assert!(!features.chess && !features.schedule);
        assert!(features.news && features.music && features.games);
    }

    #[test]
    fn feature_entries_follow_the_toml_keys() {
        let features = Features {
            music: false,
            ..Features::default()
        };
        let raw = toml::to_string(&features).unwrap();

        for (name, enabled) in features.entries() {
            assert!(raw.contains(&format!("{name} = {enabled}")), "{name}");
        }
        assert_eq!(features.entries().len(), raw.lines().count());
    }

    #[test]
    fn unknown_features_are_rejected() {
        let raw = format!("{VALID}\n[features]\nspotify = false\n");
        assert!(matches!(
            SiteConfig::parse(&raw),
            Err(SiteConfigError::Parse(_))
        ));
    }

    #[test]
    fn the_main_weight_prefers_upright_faces() {
        let mut font = valid().theme.fonts.body;
        assert_eq!(font.main_weight(), 600);

        font.faces.swap(0, 1);
        assert_eq!(font.main_weight(), 600);

        font.faces.retain(|face| face.style == FontStyle::Italic);
        assert_eq!(font.main_weight(), 400);

        font.faces.clear();
        assert_eq!(font.main_weight(), 400);
    }

    #[test]
    fn the_site_identity_is_optional() {
        let identity = valid().identity;

        assert_eq!(identity, Identity::default());
        assert_eq!(identity.name, "My digital garden");
        assert!(identity.validate().is_empty());
    }

    #[test]
    fn the_site_identity_is_read_from_the_site_table() {
        let raw = format!(
            "[site]\nname = \"Ana's garden\"\nsource_url = \"https://github.com/ana/garden\"\n\n[site.description]\nen = \"Notes\"\nes = \"Notas\"\n\n[[site.links]]\nlabel = \"MAIL\"\nurl = \"mailto:ana@example.com\"\n{VALID}"
        );
        let identity = SiteConfig::parse(&raw).unwrap().identity;

        assert_eq!(identity.name, "Ana's garden");
        assert_eq!(
            identity.source_url.as_deref(),
            Some("https://github.com/ana/garden")
        );
        assert_eq!(identity.description["es"], "Notas");
        assert_eq!(identity.links[0].url, "mailto:ana@example.com");
    }

    #[test]
    fn identity_values_are_checked() {
        let link = |label: &str, url: &str| Link {
            label: label.to_string(),
            url: url.to_string(),
        };
        let bad = Identity {
            name: " ".to_string(),
            source_url: Some("javascript:alert(1)".to_string()),
            description: BTreeMap::from([
                ("EN".to_string(), "fine".to_string()),
                ("es".to_string(), "line\nbreak".to_string()),
            ]),
            links: vec![
                link("{{CONTENT}}", "https://github.com/a\"onmouseover=x"),
                link("", "https://ok.example"),
                link("FINE", "https://"),
            ],
        };

        let problems = bad.validate();
        assert_eq!(problems.len(), 8, "{problems:#?}");
        assert!(problems[0].starts_with("site.name"));
        assert!(problems[1].starts_with("site.source_url"));

        let too_many = Identity {
            links: vec![link("A", "https://a.example"); MAX_LINKS + 1],
            ..Identity::default()
        };
        assert_eq!(
            too_many.validate(),
            vec!["site.links can have at most 8 links"]
        );

        let good = Identity {
            name: "Ñandú's jardín".to_string(),
            source_url: Some("http://localhost:3000/x?y=1#z".to_string()),
            description: BTreeMap::from([("pt-br".to_string(), "Olá".to_string())]),
            links: vec![link("MAIL", "mailto:me@example.com")],
        };
        assert!(good.validate().is_empty());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let raw = VALID.replace("shade = ", "glow = \"#000000\"\nshade = ");
        assert!(matches!(
            SiteConfig::parse(&raw),
            Err(SiteConfigError::Parse(_))
        ));
    }

    #[test]
    fn unknown_fallbacks_are_rejected() {
        let raw = VALID.replace("\"serif\"", "\"fantasy\"");
        assert!(matches!(
            SiteConfig::parse(&raw),
            Err(SiteConfigError::Parse(_))
        ));
    }

    #[test]
    fn colors_must_be_six_digit_hex() {
        for bad in ["f58a07", "#f58a0", "#f58a0g", "red", "#f58a07;}body{", ""] {
            let problems = problems_after(|c| c.theme.colors.primary = bad.to_string());
            assert_eq!(problems.len(), 1, "{bad}");
            assert!(problems[0].starts_with("theme.colors.primary"), "{bad}");
        }
        assert!(problems_after(|c| c.theme.colors.primary = "#ABCdef".to_string()).is_empty());
    }

    #[test]
    fn font_families_cannot_break_out_of_css_strings() {
        for bad in [
            "",
            " Iosevka",
            "Iosevka\"",
            "Ios;evka",
            "Iosevka}",
            &"a".repeat(65),
        ] {
            let problems = problems_after(|c| c.theme.fonts.body.family = bad.to_string());
            assert_eq!(problems.len(), 1, "{bad}");
        }
        assert!(
            problems_after(|c| c.theme.fonts.body.family = "JetBrainsMono Nerd_Font-2".to_string())
                .is_empty()
        );
    }

    #[test]
    fn font_sources_stay_inside_the_fonts_folder() {
        for bad in [
            "iosevka/regular.woff2",
            "fonts/regular.ttf",
            "fonts/../style.css.woff2",
            "fonts//regular.woff2",
            "fonts/./regular.woff2",
            "/fonts/regular.woff2",
            "fonts/a b.woff2",
            "fonts/a\").woff2",
            "fonts",
        ] {
            let problems = problems_after(|c| c.theme.fonts.body.faces[0].src = bad.to_string());
            assert_eq!(problems.len(), 1, "{bad}");
        }
        assert!(
            problems_after(|c| c.theme.fonts.body.faces[0].src = "fonts/x.woff2".to_string())
                .is_empty()
        );
    }

    #[test]
    fn font_weights_and_ranges_are_checked() {
        assert_eq!(
            problems_after(|c| c.theme.fonts.body.faces[0].weight = 950).len(),
            1
        );
        assert_eq!(
            problems_after(|c| c.theme.fonts.body.faces[0].weight = 50).len(),
            1
        );
        for bad in [
            "0000-00FF",
            "U+",
            "U+0000-",
            "U+00?0-00FF",
            "U+1234567",
            "U+00FF;}",
        ] {
            let problems = problems_after(|c| {
                c.theme.fonts.body.faces[0].unicode_range = Some(bad.to_string())
            });
            assert_eq!(problems.len(), 1, "{bad}");
        }
    }

    #[test]
    fn every_font_needs_at_least_one_face() {
        let problems = problems_after(|c| c.theme.fonts.heading.faces.clear());
        assert_eq!(
            problems,
            vec!["theme.fonts.heading.faces must not be empty"]
        );
    }

    #[test]
    fn all_problems_are_reported_at_once() {
        let raw = VALID
            .replace("#1e242c", "black")
            .replace("\"Iosevka\"", "\"Ios\\\"evka\"");
        let Err(SiteConfigError::Invalid(problems)) = SiteConfig::parse(&raw) else {
            panic!("expected validation errors");
        };
        assert_eq!(problems.len(), 2);
    }

    #[test]
    fn missing_font_files_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("fonts/iosevka")).unwrap();
        std::fs::write(dir.path().join("fonts/iosevka/regular.woff2"), b"").unwrap();

        let missing = valid().missing_font_files(dir.path());

        assert_eq!(
            missing,
            vec![
                dir.path().join("fonts/iosevka/italic.woff2"),
                dir.path().join("fonts/tusker-grotesk/regular.woff2"),
            ]
        );
    }

    #[test]
    fn loading_reports_unreadable_files_and_missing_fonts() {
        assert!(matches!(
            SiteConfig::load("does-not-exist.toml", REPO_STATIC_DIR),
            Err(SiteConfigError::Read(_))
        ));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site.toml");
        std::fs::write(&path, VALID).unwrap();
        let Err(SiteConfigError::Invalid(problems)) = SiteConfig::load(&path, dir.path()) else {
            panic!("expected missing fonts");
        };
        assert_eq!(problems.len(), 3);
        assert!(problems[0].ends_with("does not exist"));
    }

    #[test]
    fn the_theme_style_declares_fonts_and_overrides_colors() {
        let style = valid().theme_style();

        assert!(style.starts_with("<style id=\"site-theme\">"));
        assert!(style.contains(
            "@font-face{font-family:\"Iosevka\";src:url(\"/_static/fonts/iosevka/regular.woff2\") format(\"woff2\");font-weight:600;font-style:normal;font-display:swap}"
        ));
        assert!(style.contains(
            "font-weight:400;font-style:italic;font-display:swap;unicode-range:U+0000-00FF, U+0131, U+02??}"
        ));
        assert!(style.contains("--color-primary-color:#f58a07;"));
        assert!(style.contains("--color-background-color:#1e242c;"));
        assert!(style.contains("--font-body:\"Iosevka\",sans-serif;"));
        assert!(style.contains("--font-heading:\"Tusker Grotesk\",serif;"));
        assert!(style.contains("--font-body-weight:600;"));
        assert!(style.contains("--font-heading-weight:400;"));
    }

    #[test]
    fn configs_round_trip_through_toml() {
        let config = valid();
        let raw = toml::to_string(&config).unwrap();
        assert_eq!(SiteConfig::parse(&raw).unwrap(), config);
    }

    #[test]
    fn errors_are_readable() {
        let invalid = SiteConfigError::Invalid(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(invalid.to_string(), "a; b");
        assert!(
            SiteConfig::parse("nope")
                .unwrap_err()
                .to_string()
                .starts_with("could not parse TOML")
        );
    }
}
