use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use site_config::settings::{
    FONTSOURCE_API_URL, FONTSOURCE_CDN_URL, FontRequest, SettingsRequest, is_fontsource_id,
    is_offered_font,
};
use site_config::{Font, FontFace, FontFallback, FontStyle, SiteConfig};

pub const DEFAULT_CONFIG_PATH: &str = "site.toml";
pub const DEFAULT_STATIC_DIR: &str = "static";
const PREFERRED_SUBSETS: [&str; 2] = ["latin", "latin-ext"];
pub const LARGE_FONT_FILE_BYTES: u64 = 300 * 1024;
const FONTS_DIR: &str = "fonts";
const WOFF2_SIGNATURE: &[u8] = b"wOF2";
const MAX_FONT_FILE_BYTES: usize = 2 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FontDetails {
    id: String,
    family: String,
    subsets: Vec<String>,
    weights: Vec<u16>,
    def_subset: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    unicode_range: HashMap<String, String>,
}

pub struct Fontsource {
    client: reqwest::Client,
    api_url: String,
    cdn_url: String,
}

impl Fontsource {
    pub fn new(api_url: impl Into<String>, cdn_url: impl Into<String>) -> Fontsource {
        Fontsource {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("a reqwest client with a timeout"),
            api_url: api_url.into(),
            cdn_url: cdn_url.into(),
        }
    }

    pub fn official() -> Fontsource {
        Fontsource::new(FONTSOURCE_API_URL, FONTSOURCE_CDN_URL)
    }

    async fn details(&self, id: &str) -> Result<FontDetails, String> {
        self.client
            .get(format!("{}/v1/fonts/{id}", self.api_url))
            .send()
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|e| format!("could not look up {id:?} on Fontsource: {e}"))?
            .json()
            .await
            .map_err(|e| format!("unexpected Fontsource response for {id:?}: {e}"))
    }

    async fn download(&self, id: &str, subset: &str, weight: u16) -> Result<Vec<u8>, String> {
        let url = format!(
            "{}/{id}@latest/{subset}-{weight}-normal.woff2",
            self.cdn_url
        );
        let bytes = self
            .client
            .get(&url)
            .send()
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|e| format!("could not download {url}: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("could not download {url}: {e}"))?;

        if bytes.len() > MAX_FONT_FILE_BYTES {
            return Err(format!("{url} is larger than {MAX_FONT_FILE_BYTES} bytes"));
        }
        if !bytes.starts_with(WOFF2_SIGNATURE) {
            return Err(format!("{url} is not a WOFF2 file"));
        }
        Ok(bytes.to_vec())
    }
}

#[derive(Debug)]
pub struct Outcome {
    pub config: SiteConfig,
    pub added: Vec<PathBuf>,
    pub removed: Vec<Removed>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub path: PathBuf,
    pub is_folder: bool,
}

pub async fn apply(
    request: &SettingsRequest,
    current: &SiteConfig,
    static_dir: &Path,
    fontsource: &Fontsource,
    issue: Option<u64>,
) -> Result<Outcome, Vec<String>> {
    let problems = request.validate();
    if !problems.is_empty() {
        return Err(problems);
    }
    if !request.changes_anything(current) {
        return Err(vec![
            "the request does not change anything in site.toml".to_string(),
        ]);
    }

    let mut config = current.clone();
    config.identity = request.identity.clone();
    config.theme.colors = request.colors.clone();
    config.features = request.features;
    if let Some(radius) = request.radius {
        config.theme.radius = radius;
    }
    if let FontRequest::Keep {
        fallback: Some(fallback),
    } = request.fonts.body
    {
        config.theme.fonts.body.fallback = fallback;
    }
    if let FontRequest::Keep {
        fallback: Some(fallback),
    } = request.fonts.heading
    {
        config.theme.fonts.heading.fallback = fallback;
    }

    let mut added = Vec::new();
    if let FontRequest::Fontsource {
        id,
        weight,
        fallback,
    } = &request.fonts.body
    {
        config.theme.fonts.body =
            install_font(fontsource, static_dir, id, *weight, *fallback, &mut added)
                .await
                .map_err(|e| vec![format!("fonts.body: {e}")])?;
    }
    if let FontRequest::Fontsource {
        id,
        weight,
        fallback,
    } = &request.fonts.heading
    {
        config.theme.fonts.heading =
            install_font(fontsource, static_dir, id, *weight, *fallback, &mut added)
                .await
                .map_err(|e| vec![format!("fonts.heading: {e}")])?;
    }

    let mut problems = config.validate();
    problems.extend(
        config
            .missing_font_files(static_dir)
            .into_iter()
            .map(|path| format!("font file {} does not exist", path.display())),
    );
    if !problems.is_empty() {
        return Err(problems);
    }

    let removed = prune_unused_fonts(&config, static_dir).map_err(|e| vec![e])?;
    let summary = summarize(current, &config, &added, &removed, static_dir, issue);

    Ok(Outcome {
        config,
        added,
        removed,
        summary,
    })
}

async fn install_font(
    fontsource: &Fontsource,
    static_dir: &Path,
    id: &str,
    weight: u16,
    fallback: FontFallback,
    added: &mut Vec<PathBuf>,
) -> Result<Font, String> {
    let details = fontsource.details(id).await?;
    if details.id != id || !is_offered_font(&details.kind, &details.category) {
        return Err(format!(
            "{id:?} is not a text font from Google Fonts or Fontsource's open-source fonts"
        ));
    }
    if !details.weights.contains(&weight) {
        return Err(format!("{} has no weight {weight}", details.family));
    }

    let mut subsets: Vec<&str> = PREFERRED_SUBSETS
        .into_iter()
        .filter(|subset| details.subsets.iter().any(|offered| offered == subset))
        .collect();
    if subsets.is_empty() {
        subsets.push(&details.def_subset);
    }

    let mut faces = Vec::new();
    for subset in subsets {
        if !is_fontsource_id(subset) {
            return Err(format!("unexpected subset name {subset:?}"));
        }
        let bytes = fontsource.download(id, subset, weight).await?;
        let src = format!("{FONTS_DIR}/{id}/{subset}-{weight}-normal.woff2");
        let path = static_dir.join(&src);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, bytes)
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        if !added.contains(&path) {
            added.push(path);
        }

        faces.push(FontFace {
            src,
            weight,
            style: FontStyle::Normal,
            unicode_range: details.unicode_range.get(subset).cloned(),
        });
    }

    Ok(Font {
        family: details.family,
        fallback,
        faces,
    })
}

fn prune_unused_fonts(config: &SiteConfig, static_dir: &Path) -> Result<Vec<Removed>, String> {
    let used: BTreeSet<PathBuf> = [&config.theme.fonts.body, &config.theme.fonts.heading]
        .into_iter()
        .flat_map(|font| &font.faces)
        .map(|face| static_dir.join(&face.src))
        .collect();

    let mut removed = Vec::new();
    for folder in children(&static_dir.join(FONTS_DIR)) {
        if !folder.is_dir() {
            continue;
        }
        let files = files_under(&folder);

        if !files.iter().any(|file| used.contains(file)) {
            std::fs::remove_dir_all(&folder)
                .map_err(|e| format!("could not remove {}: {e}", folder.display()))?;
            removed.push(Removed {
                path: folder,
                is_folder: true,
            });
            continue;
        }

        for file in files {
            if used.contains(&file) || !is_font_file(&file) {
                continue;
            }
            std::fs::remove_file(&file)
                .map_err(|e| format!("could not remove {}: {e}", file.display()))?;
            removed.push(Removed {
                path: file,
                is_folder: false,
            });
        }
    }
    removed.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(removed)
}

fn children(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default()
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    children(dir)
        .into_iter()
        .flat_map(|path| {
            if path.is_dir() {
                files_under(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "woff2")
}

fn summarize(
    before: &SiteConfig,
    after: &SiteConfig,
    added: &[PathBuf],
    removed: &[Removed],
    static_dir: &Path,
    issue: Option<u64>,
) -> String {
    let mut out = String::new();
    if let Some(issue) = issue {
        out.push_str(&format!("Settings requested in #{issue}.\n\n"));
    }

    let identity = identity_changes(before, after);
    if !identity.is_empty() {
        out.push_str("**Site**\n\n");
        out.push_str(&identity.join("\n"));
        out.push_str("\n\n");
    }

    let colors: Vec<String> = before
        .theme
        .colors
        .entries()
        .into_iter()
        .zip(after.theme.colors.entries())
        .filter(|((_, old), (_, new))| old != new)
        .map(|((name, old), (_, new))| format!("| {name} | `{old}` | `{new}` |"))
        .collect();
    if !colors.is_empty() {
        out.push_str("**Colors**\n\n| Color | Before | After |\n| --- | --- | --- |\n");
        out.push_str(&colors.join("\n"));
        out.push_str("\n\n");
    }

    if before.theme.radius != after.theme.radius {
        out.push_str(&format!(
            "**Corners**\n\n- radius: {}px → {}px\n\n",
            before.theme.radius, after.theme.radius
        ));
    }

    let fonts: Vec<String> = before
        .theme
        .fonts
        .entries()
        .into_iter()
        .zip(after.theme.fonts.entries())
        .filter(|((_, old), (_, new))| old != new)
        .map(|((role, old), (_, new))| {
            format!(
                "- {role}: {} → {} (weight {}, {} fallback)",
                old.family,
                new.family,
                new.main_weight(),
                new.fallback.as_css()
            )
        })
        .collect();
    if !fonts.is_empty() {
        out.push_str("**Fonts**\n\n");
        out.push_str(&fonts.join("\n"));
        out.push_str("\n\n");
    }

    let switch = |on: bool| if on { "on" } else { "off" };
    let features: Vec<String> = before
        .features
        .entries()
        .into_iter()
        .zip(after.features.entries())
        .filter(|((_, old), (_, new))| old != new)
        .map(|((name, old), (_, new))| format!("- {name}: {} → {}", switch(old), switch(new)))
        .collect();
    if !features.is_empty() {
        out.push_str("**Sections**\n\n");
        out.push_str(&features.join("\n"));
        out.push_str("\n\n");
    }

    let folder = static_dir
        .file_name()
        .map_or_else(|| DEFAULT_STATIC_DIR.into(), |name| name.to_string_lossy());
    let shown = |path: &PathBuf| {
        let relative = path.strip_prefix(static_dir).unwrap_or(path);
        format!("{folder}/{}", relative.display())
    };
    if !added.is_empty() || !removed.is_empty() {
        out.push_str("**Font files**\n\n");
        let mut large = false;
        for path in added {
            let bytes = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
            large |= bytes > LARGE_FONT_FILE_BYTES;
            out.push_str(&format!(
                "- added `{}` ({} KB)\n",
                shown(path),
                bytes.div_ceil(1024)
            ));
        }
        for item in removed {
            let slash = if item.is_folder { "/" } else { "" };
            out.push_str(&format!("- removed `{}{slash}`\n", shown(&item.path)));
        }
        out.push('\n');
        if large {
            out.push_str(&format!(
                "**Heads-up:** some font files are larger than {} KB, so pages will take longer to load. \
                 Fonts from Google Fonts are split by alphabet and are usually much smaller.\n\n",
                LARGE_FONT_FILE_BYTES / 1024
            ));
        }
    }

    if let Some(issue) = issue {
        out.push_str(&format!("Closes #{issue}\n"));
    }
    out
}

fn identity_changes(before: &SiteConfig, after: &SiteConfig) -> Vec<String> {
    let (old, new) = (&before.identity, &after.identity);
    let mut changes = Vec::new();

    if old.name != new.name {
        changes.push(format!("- name: {} → {}", old.name, new.name));
    }
    if old.source_url != new.source_url {
        changes.push(format!(
            "- source link: {} → {}",
            old.source_url.as_deref().unwrap_or("none"),
            new.source_url.as_deref().unwrap_or("none")
        ));
    }
    let langs: BTreeSet<&String> = old
        .description
        .keys()
        .chain(new.description.keys())
        .collect();
    for lang in langs {
        if old.description.get(lang) != new.description.get(lang) {
            changes.push(format!(
                "- description ({lang}): {}",
                new.description
                    .get(lang)
                    .map(String::as_str)
                    .unwrap_or("removed")
            ));
        }
    }
    if old.links != new.links {
        let labels: Vec<&str> = new.links.iter().map(|link| link.label.as_str()).collect();
        changes.push(format!("- footer links: {}", labels.join(", ")));
    }

    changes
}

pub fn render_config(config: &SiteConfig) -> String {
    toml::to_string_pretty(config).expect("site configs always serialize")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub request: PathBuf,
    pub summary: PathBuf,
    pub issue: Option<u64>,
    pub config: PathBuf,
    pub static_dir: PathBuf,
}

impl Args {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
        let mut request = None;
        let mut summary = None;
        let mut issue = None;
        let mut config = PathBuf::from(DEFAULT_CONFIG_PATH);
        let mut static_dir = PathBuf::from(DEFAULT_STATIC_DIR);

        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "--request" => request = Some(PathBuf::from(value()?)),
                "--summary" => summary = Some(PathBuf::from(value()?)),
                "--config" => config = PathBuf::from(value()?),
                "--static-dir" => static_dir = PathBuf::from(value()?),
                "--issue" => {
                    let raw = value()?;
                    issue = Some(
                        raw.parse()
                            .map_err(|_| format!("--issue must be a number, got {raw:?}"))?,
                    );
                }
                other => return Err(format!("unknown argument {other:?}")),
            }
        }

        Ok(Args {
            request: request.ok_or("--request is required")?,
            summary: summary.ok_or("--summary is required")?,
            issue,
            config,
            static_dir,
        })
    }
}

pub async fn run(args: &Args, fontsource: &Fontsource) -> Result<Outcome, Vec<String>> {
    let read = |path: &Path| {
        std::fs::read_to_string(path)
            .map_err(|e| vec![format!("could not read {}: {e}", path.display())])
    };

    let request = SettingsRequest::from_issue_body(&read(&args.request)?)?;
    let current = SiteConfig::load(&args.config, &args.static_dir).map_err(|e| {
        vec![format!(
            "the current {} is invalid: {e}",
            args.config.display()
        )]
    })?;

    let outcome = apply(&request, &current, &args.static_dir, fontsource, args.issue).await?;

    let write = |path: &Path, text: &str| {
        std::fs::write(path, text)
            .map_err(|e| vec![format!("could not write {}: {e}", path.display())])
    };
    write(&args.config, &render_config(&outcome.config))?;
    write(&args.summary, &outcome.summary)?;
    Ok(outcome)
}
