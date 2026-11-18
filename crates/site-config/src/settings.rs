use serde::{Deserialize, Serialize};

use crate::{Colors, Features, FontFallback, Identity, MAX_RADIUS, SiteConfig, is_hex_color};

pub const ISSUE_TITLE: &str = "Site settings update";
pub const ISSUE_LABEL: &str = "site-settings";
pub const REQUEST_FENCE: &str = "```toml site-settings";
pub const MAX_ISSUE_URL_LEN: usize = 8000;
const FENCE_END: &str = "```";
pub const FONTSOURCE_API_URL: &str = "https://api.fontsource.org";
pub const FONTSOURCE_CDN_URL: &str = "https://cdn.jsdelivr.net/fontsource/fonts";
pub const OFFERED_FONT_TYPES: [&str; 2] = ["google", "other"];
pub const HIDDEN_FONT_CATEGORIES: [&str; 2] = ["icons", "other"];
const MAX_FONT_ID_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<u8>,
    #[serde(rename = "site")]
    pub identity: Identity,
    pub colors: Colors,
    pub fonts: FontRequests,
    pub features: Features,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontRequests {
    pub body: FontRequest,
    pub heading: FontRequest,
}

impl FontRequests {
    pub fn entries(&self) -> [(&'static str, &FontRequest); 2] {
        [("body", &self.body), ("heading", &self.heading)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FontRequest {
    Keep {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<FontFallback>,
    },
    Fontsource {
        id: String,
        weight: u16,
        fallback: FontFallback,
    },
}

impl FontRequest {
    pub fn keep() -> FontRequest {
        FontRequest::Keep { fallback: None }
    }
}

impl SettingsRequest {
    pub fn keeping(site: &SiteConfig) -> SettingsRequest {
        SettingsRequest {
            radius: None,
            identity: site.identity.clone(),
            colors: site.theme.colors.clone(),
            fonts: FontRequests {
                body: FontRequest::keep(),
                heading: FontRequest::keep(),
            },
            features: site.features,
        }
    }

    pub fn changes_anything(&self, site: &SiteConfig) -> bool {
        let request = SettingsRequest {
            radius: self.radius.filter(|radius| *radius != site.theme.radius),
            ..self.clone()
        };
        request != SettingsRequest::keeping(site)
    }

    pub fn parse(raw: &str) -> Result<SettingsRequest, Vec<String>> {
        let request: SettingsRequest =
            toml::from_str(raw).map_err(|e| vec![format!("could not parse the request: {e}")])?;
        let problems = request.validate();
        if problems.is_empty() {
            Ok(request)
        } else {
            Err(problems)
        }
    }

    pub fn from_issue_body(body: &str) -> Result<SettingsRequest, Vec<String>> {
        let mut lines = body.lines().skip_while(|line| line.trim() != REQUEST_FENCE);
        if lines.next().is_none() {
            return Err(vec![format!("no {REQUEST_FENCE} block found")]);
        }

        let mut raw = String::new();
        for line in lines {
            if line.trim() == FENCE_END {
                return SettingsRequest::parse(&raw);
            }
            raw.push_str(line);
            raw.push('\n');
        }
        Err(vec![format!("the {REQUEST_FENCE} block is not closed")])
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("settings requests always serialize")
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = self.identity.validate();

        if let Some(radius) = self.radius
            && radius > MAX_RADIUS
        {
            problems.push(format!(
                "radius must be between 0 and {MAX_RADIUS} pixels, got {radius}"
            ));
        }

        for (name, value) in self.colors.entries() {
            if !is_hex_color(value) {
                problems.push(format!(
                    "colors.{name} must be a #rrggbb color, got {value:?}"
                ));
            }
        }

        for (role, font) in self.fonts.entries() {
            if let FontRequest::Fontsource { id, weight, .. } = font {
                if !is_fontsource_id(id) {
                    problems.push(format!(
                        "fonts.{role}.id must be a Fontsource id like \"open-sans\", got {id:?}"
                    ));
                }
                if !is_font_weight(*weight) {
                    problems.push(format!(
                        "fonts.{role}.weight must be one of 100, 200, ... 900, got {weight}"
                    ));
                }
            }
        }

        problems
    }
}

pub fn is_fontsource_id(id: &str) -> bool {
    (1..=MAX_FONT_ID_LEN).contains(&id.len())
        && !id.starts_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn is_offered_font(kind: &str, category: &str) -> bool {
    OFFERED_FONT_TYPES.contains(&kind) && !HIDDEN_FONT_CATEGORIES.contains(&category)
}

pub fn is_font_weight(weight: u16) -> bool {
    (100..=900).contains(&weight) && weight.is_multiple_of(100)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRepo {
    pub owner: String,
    pub name: String,
}

impl GithubRepo {
    pub fn from_repo_source(repo_source: &str) -> Option<GithubRepo> {
        let path = repo_source.trim().strip_prefix("https://github.com/")?;
        let mut segments = path.split('/');
        let owner = segments.next()?;
        let name = segments.next()?;
        let name = name.strip_suffix(".git").unwrap_or(name);

        (is_github_name(owner) && is_github_name(name)).then(|| GithubRepo {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    pub fn url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.name)
    }

    pub fn new_issue_url(&self, request: &SettingsRequest) -> Result<String, String> {
        let body = format!(
            "This request was created from the admin settings page.\n\
             A workflow will validate it and open a pull request with the new settings.\n\n\
             {REQUEST_FENCE}\n{}{FENCE_END}\n",
            request.to_toml()
        );
        let url = format!(
            "{}/issues/new?title={}&labels={}&body={}",
            self.url(),
            encode_query_value(ISSUE_TITLE),
            encode_query_value(ISSUE_LABEL),
            encode_query_value(&body)
        );

        if url.len() > MAX_ISSUE_URL_LEN {
            Err(format!(
                "the settings request is too long for a GitHub link ({} characters)",
                url.len()
            ))
        } else {
            Ok(url)
        }
    }
}

fn is_github_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> SiteConfig {
        SiteConfig::load(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../e2e/fixtures/site.toml"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../static"),
        )
        .unwrap()
    }

    fn request() -> SettingsRequest {
        SettingsRequest {
            fonts: FontRequests {
                body: FontRequest::Fontsource {
                    id: "inter".to_string(),
                    weight: 600,
                    fallback: FontFallback::SansSerif,
                },
                heading: FontRequest::keep(),
            },
            features: Features {
                chess: false,
                ..Features::default()
            },
            ..SettingsRequest::keeping(&site())
        }
    }

    fn decode(value: &str) -> String {
        let bytes = value.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                out.push(u8::from_str_radix(&value[i + 1..i + 3], 16).unwrap());
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn requests_round_trip_through_toml() {
        let request = request();
        let raw = request.to_toml();

        assert!(raw.contains("[fonts.body]\nsource = \"fontsource\"\nid = \"inter\""));
        assert!(raw.contains("[fonts.heading]\nsource = \"keep\""));
        assert!(!raw.contains("[fonts.heading]\nsource = \"keep\"\nfallback"));
        assert!(raw.contains("chess = false"));
        assert_eq!(SettingsRequest::parse(&raw).unwrap(), request);
    }

    #[test]
    fn kept_fonts_can_change_their_fallback() {
        let request = SettingsRequest {
            fonts: FontRequests {
                body: FontRequest::keep(),
                heading: FontRequest::Keep {
                    fallback: Some(FontFallback::Serif),
                },
            },
            ..SettingsRequest::keeping(&site())
        };

        let raw = request.to_toml();
        assert!(raw.contains("[fonts.heading]\nsource = \"keep\"\nfallback = \"serif\""));
        assert_eq!(SettingsRequest::parse(&raw).unwrap(), request);
        assert!(request.changes_anything(&site()));
    }

    #[test]
    fn keeping_the_site_changes_nothing() {
        let site = site();

        assert!(!SettingsRequest::keeping(&site).changes_anything(&site));
        assert!(request().changes_anything(&site));
    }

    #[test]
    fn the_radius_is_optional_and_only_counts_when_it_changes() {
        let site = site();
        let keeping = SettingsRequest::keeping(&site);
        assert!(!keeping.to_toml().contains("radius"));

        let same = SettingsRequest {
            radius: Some(site.theme.radius),
            ..keeping.clone()
        };
        assert!(!same.changes_anything(&site));

        let rounder = SettingsRequest {
            radius: Some(site.theme.radius + 8),
            ..keeping
        };
        assert!(rounder.changes_anything(&site));
        let raw = rounder.to_toml();
        assert!(raw.starts_with(&format!("radius = {}\n", site.theme.radius + 8)));
        assert_eq!(SettingsRequest::parse(&raw).unwrap(), rounder);
    }

    #[test]
    fn the_requested_radius_is_limited() {
        let request = SettingsRequest {
            radius: Some(MAX_RADIUS + 1),
            ..SettingsRequest::keeping(&site())
        };
        assert_eq!(
            request.validate(),
            vec!["radius must be between 0 and 24 pixels, got 25"]
        );
        assert!(
            SettingsRequest::parse(&request.to_toml().replace("radius = 25", "radius = -1"))
                .unwrap_err()[0]
                .starts_with("could not parse the request")
        );
    }

    #[test]
    fn invalid_values_are_all_reported() {
        let mut request = request();
        request.colors.primary = "orange".to_string();
        request.fonts.heading = FontRequest::Fontsource {
            id: "../Inter".to_string(),
            weight: 650,
            fallback: FontFallback::Serif,
        };

        let problems = request.validate();
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].starts_with("colors.primary"));
        assert!(problems[1].starts_with("fonts.heading.id"));
        assert!(problems[2].starts_with("fonts.heading.weight"));
        assert!(SettingsRequest::parse(&request.to_toml()).is_err());
    }

    #[test]
    fn unknown_fields_and_sources_are_rejected() {
        let raw = request().to_toml();

        for bad in [
            raw.replace("source = \"keep\"", "source = \"upload\""),
            raw.replace("source = \"keep\"", "source = \"keep\"\nid = \"inter\""),
            format!("{raw}\n[extra]\nvalue = 1\n"),
        ] {
            assert!(SettingsRequest::parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn fontsource_ids_and_weights_are_checked() {
        for good in ["inter", "open-sans", "42dot-sans"] {
            assert!(is_fontsource_id(good), "{good}");
        }
        for bad in [
            "",
            "Inter",
            "open sans",
            "-inter",
            "inter/../x",
            &"a".repeat(65),
        ] {
            assert!(!is_fontsource_id(bad), "{bad}");
        }
        assert!(is_font_weight(100) && is_font_weight(900));
        assert!(!is_font_weight(0) && !is_font_weight(950) && !is_font_weight(450));
    }

    #[test]
    fn google_and_open_source_text_fonts_are_offered() {
        assert!(is_offered_font("google", "sans-serif"));
        assert!(is_offered_font("other", "monospace"));
        assert!(is_offered_font("other", "display"));
        assert!(!is_offered_font("google", "icons"));
        assert!(!is_offered_font("other", "other"));
        assert!(!is_offered_font("commercial", "serif"));
    }

    #[test]
    fn github_repositories_are_read_from_the_repo_source() {
        let repo =
            GithubRepo::from_repo_source("https://github.com/Chemchu/garden-engine/blob/main")
                .unwrap();
        assert_eq!(repo.owner, "Chemchu");
        assert_eq!(repo.name, "garden-engine");
        assert_eq!(repo.url(), "https://github.com/Chemchu/garden-engine");

        assert_eq!(
            GithubRepo::from_repo_source("https://github.com/some-one/my.garden.git")
                .unwrap()
                .name,
            "my.garden"
        );

        for bad in [
            "test_repo_source",
            "https://gitlab.com/a/b",
            "https://github.com/onlyowner",
            "https://github.com//repo",
            "https://github.com/a/..",
            "https://github.com/a b/c",
        ] {
            assert!(GithubRepo::from_repo_source(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn the_issue_link_carries_the_request_in_a_fenced_block() {
        let repo = GithubRepo::from_repo_source("https://github.com/a/b").unwrap();
        let request = request();

        let url = repo.new_issue_url(&request).unwrap();
        assert!(url.starts_with(
            "https://github.com/a/b/issues/new?title=Site%20settings%20update&labels=site-settings&body="
        ));

        let body = decode(url.split("&body=").nth(1).unwrap());
        assert!(body.contains(REQUEST_FENCE));
        assert_eq!(SettingsRequest::from_issue_body(&body).unwrap(), request);
    }

    #[test]
    fn issue_bodies_need_a_closed_request_block() {
        assert!(SettingsRequest::from_issue_body("no request here").is_err());
        assert!(SettingsRequest::from_issue_body(&format!("{REQUEST_FENCE}\n[colors]\n")).is_err());
        let invalid = format!("{REQUEST_FENCE}\nnot toml\n```\n");
        assert!(SettingsRequest::from_issue_body(&invalid).is_err());
    }

    #[test]
    fn query_values_are_fully_encoded() {
        assert_eq!(
            encode_query_value("a b&c=d#ñ\n"),
            "a%20b%26c%3Dd%23%C3%B1%0A"
        );
    }
}
