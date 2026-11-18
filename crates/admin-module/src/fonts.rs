use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use site_config::settings::{FONTSOURCE_API_URL, is_offered_font};
use tokio::sync::Mutex;

const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const FAILURE_TTL: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const GOOGLE_TYPE: &str = "google";
const DEFAULT_SUBSET: &str = "latin";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogFont {
    pub id: String,
    pub family: String,
    pub weights: Vec<u16>,
    pub category: String,
    #[serde(
        rename(deserialize = "defSubset", serialize = "subset"),
        default = "default_subset"
    )]
    pub subset: String,
    #[serde(rename = "type", skip_serializing)]
    pub kind: String,
}

impl CatalogFont {
    pub fn is_google(&self) -> bool {
        self.kind == GOOGLE_TYPE
    }

    pub fn has_weight(&self, weight: u16) -> bool {
        self.weights.contains(&weight)
    }

    pub fn closest_weight(&self, wanted: u16) -> Option<u16> {
        self.weights
            .iter()
            .copied()
            .min_by_key(|weight| (weight.abs_diff(wanted), *weight))
    }
}

fn default_subset() -> String {
    DEFAULT_SUBSET.to_string()
}

type CatalogResult = Result<Arc<Vec<CatalogFont>>, String>;

struct Cached {
    fetched: Instant,
    result: CatalogResult,
}

pub struct FontCatalog {
    client: reqwest::Client,
    base_url: String,
    cache: Mutex<Option<Cached>>,
    failure_ttl: Duration,
}

impl Default for FontCatalog {
    fn default() -> Self {
        FontCatalog::new(FONTSOURCE_API_URL)
    }
}

impl FontCatalog {
    pub fn new(base_url: impl Into<String>) -> FontCatalog {
        FontCatalog {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("a reqwest client with a timeout"),
            base_url: base_url.into(),
            cache: Mutex::new(None),
            failure_ttl: FAILURE_TTL,
        }
    }

    pub async fn fonts(&self) -> CatalogResult {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref() {
            let ttl = if cached.result.is_ok() {
                CACHE_TTL
            } else {
                self.failure_ttl
            };
            if cached.fetched.elapsed() < ttl {
                return cached.result.clone();
            }
        }

        let result = self.fetch().await.map(Arc::new);
        *cache = Some(Cached {
            fetched: Instant::now(),
            result: result.clone(),
        });
        result
    }

    pub async fn find(&self, id_or_family: &str) -> Result<Option<CatalogFont>, String> {
        let wanted = id_or_family.trim();
        Ok(self
            .fonts()
            .await?
            .iter()
            .find(|font| font.id == wanted || font.family.eq_ignore_ascii_case(wanted))
            .cloned())
    }

    async fn fetch(&self) -> Result<Vec<CatalogFont>, String> {
        let mut fonts: Vec<CatalogFont> = self
            .client
            .get(format!("{}/v1/fonts", self.base_url))
            .send()
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|e| format!("could not reach the font catalog: {e}"))?
            .json()
            .await
            .map_err(|e| format!("unexpected font catalog response: {e}"))?;

        fonts
            .retain(|font| is_offered_font(&font.kind, &font.category) && !font.weights.is_empty());
        fonts.sort_by_key(|font| font.family.to_lowercase());
        Ok(fonts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn font(
        id: &str,
        family: &str,
        weights: &[u16],
        category: &str,
        kind: &str,
    ) -> serde_json::Value {
        json!({
            "id": id,
            "family": family,
            "subsets": ["latin"],
            "defSubset": "latin",
            "weights": weights,
            "styles": ["normal"],
            "category": category,
            "type": kind,
        })
    }

    async fn catalog_server(expected_calls: u64) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/fonts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                font("roboto", "Roboto", &[400, 700], "sans-serif", "google"),
                font(
                    "inter",
                    "Inter",
                    &[100, 400, 600, 900],
                    "sans-serif",
                    "google"
                ),
                font(
                    "material-icons",
                    "Material Icons",
                    &[400],
                    "icons",
                    "google"
                ),
                font("iosevka", "Iosevka", &[400, 600], "monospace", "other"),
                font("dseg7-classic", "DSEG7 Classic", &[400], "other", "other"),
                font("commercial", "Commercial", &[400], "serif", "commercial"),
                font("empty", "Empty", &[], "serif", "google"),
            ])))
            .expect(expected_calls)
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn google_and_open_source_text_fonts_are_offered_sorted_by_family() {
        let server = catalog_server(1).await;
        let catalog = FontCatalog::new(server.uri());

        let ids: Vec<String> = catalog
            .fonts()
            .await
            .unwrap()
            .iter()
            .map(|font| font.id.clone())
            .collect();

        assert_eq!(ids, vec!["inter", "iosevka", "roboto"]);
    }

    #[tokio::test]
    async fn every_font_knows_which_subset_to_preview() {
        let server = MockServer::start().await;
        let mut khmer = font("khmer", "Khmer", &[400], "sans-serif", "google");
        khmer["subsets"] = json!(["khmer"]);
        khmer["defSubset"] = json!("khmer");
        let mut older = font("older", "Older", &[400], "serif", "google");
        older.as_object_mut().unwrap().remove("defSubset");
        Mock::given(method("GET"))
            .and(path("/v1/fonts"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([khmer, older])))
            .mount(&server)
            .await;

        let fonts = FontCatalog::new(server.uri()).fonts().await.unwrap();
        let subsets: Vec<(&str, &str)> = fonts
            .iter()
            .map(|font| (font.id.as_str(), font.subset.as_str()))
            .collect();

        assert_eq!(subsets, vec![("khmer", "khmer"), ("older", "latin")]);
    }

    #[tokio::test]
    async fn the_catalog_is_fetched_once_and_cached() {
        let server = catalog_server(1).await;
        let catalog = FontCatalog::new(server.uri());

        catalog.fonts().await.unwrap();
        catalog.fonts().await.unwrap();
        assert!(catalog.find("inter").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn fonts_are_found_by_id_or_family() {
        let server = catalog_server(1).await;
        let catalog = FontCatalog::new(server.uri());

        assert_eq!(
            catalog.find("inter").await.unwrap().unwrap().family,
            "Inter"
        );
        assert_eq!(
            catalog.find(" roboto ").await.unwrap().unwrap().id,
            "roboto"
        );
        assert_eq!(catalog.find("ROBOTO").await.unwrap().unwrap().id, "roboto");
        assert!(catalog.find("material-icons").await.unwrap().is_none());
        assert!(catalog.find("DSEG7 Classic").await.unwrap().is_none());
        assert!(!catalog.find("iosevka").await.unwrap().unwrap().is_google());
        assert!(catalog.find("inter").await.unwrap().unwrap().is_google());
        assert!(catalog.find("Comic Sans").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn catalog_failures_are_remembered_for_a_while() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .expect(2)
            .mount(&server)
            .await;
        let mut catalog = FontCatalog::new(server.uri());

        assert!(
            catalog
                .fonts()
                .await
                .unwrap_err()
                .contains("could not reach")
        );
        assert!(catalog.find("inter").await.is_err());
        assert!(catalog.find("lora").await.is_err());

        catalog.failure_ttl = Duration::ZERO;
        assert!(catalog.fonts().await.is_err());
    }

    #[tokio::test]
    async fn concurrent_lookups_share_one_download() {
        let server = catalog_server(1).await;
        let catalog = Arc::new(FontCatalog::new(server.uri()));

        let lookups: Vec<_> = (0..10)
            .map(|_| {
                let catalog = catalog.clone();
                tokio::spawn(async move { catalog.fonts().await.map(|fonts| fonts.len()) })
            })
            .collect();

        for lookup in lookups {
            assert_eq!(lookup.await.unwrap(), Ok(3));
        }
    }

    #[tokio::test]
    async fn malformed_catalogs_are_reported() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let error = FontCatalog::new(server.uri()).fonts().await.unwrap_err();
        assert!(error.contains("unexpected font catalog response"));
    }

    #[test]
    fn the_closest_weight_prefers_the_lighter_one_on_ties() {
        let inter = CatalogFont {
            id: "inter".to_string(),
            family: "Inter".to_string(),
            weights: vec![400, 800],
            category: "sans-serif".to_string(),
            subset: "latin".to_string(),
            kind: "google".to_string(),
        };

        assert_eq!(inter.closest_weight(600), Some(400));
        assert_eq!(inter.closest_weight(700), Some(800));
        assert!(inter.has_weight(800) && !inter.has_weight(600));
        assert_eq!(
            CatalogFont {
                weights: vec![],
                ..inter
            }
            .closest_weight(400),
            None
        );
    }
}
