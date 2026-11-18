use std::path::{Path, PathBuf};

use search_engine::check::check_garden;
use search_engine::utils::GARDEN_DIR;

use crate::site_config::{STATIC_DIR, SiteConfig, SiteConfigError};

pub const SITE_CONFIG_FILE: &str = "site.toml";
pub const ASSETS_FOLDER: &str = "assets";
pub const CHECK_USAGE: &str = "usage: guslee check [CONTENT_DIR] [--assets ASSETS_DIR]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPaths {
    pub garden: PathBuf,
    pub site_config: PathBuf,
    pub assets: Option<PathBuf>,
}

impl ContentPaths {
    pub fn new(content_dir: impl AsRef<Path>, assets: Option<PathBuf>) -> ContentPaths {
        let dir = content_dir.as_ref();
        ContentPaths {
            garden: dir.join(GARDEN_DIR),
            site_config: dir.join(SITE_CONFIG_FILE),
            assets: assets.or_else(|| Some(dir.join(ASSETS_FOLDER)).filter(|path| path.is_dir())),
        }
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> ContentPaths {
        let content_dir = lookup("CONTENT_DIR").unwrap_or_else(|| ".".to_string());
        let mut paths = ContentPaths::new(content_dir, lookup("ASSETS_DIR").map(PathBuf::from));
        if let Some(site_config) = lookup("SITE_CONFIG_PATH") {
            paths.site_config = PathBuf::from(site_config);
        }
        paths
    }

    pub fn from_env() -> ContentPaths {
        ContentPaths::from_lookup(|var| std::env::var(var).ok().filter(|value| !value.is_empty()))
    }

    pub fn garden_path(&self) -> String {
        self.garden.to_string_lossy().into_owned()
    }

    pub fn asset_dirs(&self) -> Vec<&Path> {
        self.assets
            .iter()
            .map(PathBuf::as_path)
            .chain([Path::new(STATIC_DIR)])
            .collect()
    }

    pub fn load_site(&self) -> Result<SiteConfig, SiteConfigError> {
        SiteConfig::load_with_assets(&self.site_config, &self.asset_dirs())
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ContentReport {
    pub posts: usize,
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
}

impl ContentReport {
    pub fn passed(&self) -> bool {
        self.problems.is_empty()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        for problem in &self.problems {
            out.push_str(&format!("error: {problem}\n"));
        }
        for warning in &self.warnings {
            out.push_str(&format!("warning: {warning}\n"));
        }
        out.push_str(&format!(
            "Checked {} posts: {} problems, {} warnings\n",
            self.posts,
            self.problems.len(),
            self.warnings.len()
        ));
        out
    }
}

pub fn check_content(paths: &ContentPaths) -> ContentReport {
    let shown = paths.site_config.display();
    let mut problems = match paths.load_site() {
        Ok(_) => Vec::new(),
        Err(SiteConfigError::Invalid(list)) => list
            .into_iter()
            .map(|problem| format!("{shown}: {problem}"))
            .collect(),
        Err(other) => vec![format!("{shown}: {other}")],
    };

    let garden = check_garden(&paths.garden);
    problems.extend(garden.problems);
    ContentReport {
        posts: garden.posts,
        problems,
        warnings: garden.warnings,
    }
}

pub fn check_paths(
    args: impl IntoIterator<Item = String>,
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<ContentPaths, String> {
    let mut content_dir = None;
    let mut assets = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--assets" => {
                assets = Some(PathBuf::from(args.next().ok_or("--assets needs a folder")?))
            }
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            _ if content_dir.is_some() => return Err(format!("unexpected argument {arg}")),
            _ => content_dir = Some(arg),
        }
    }

    Ok(match content_dir {
        Some(dir) => ContentPaths::new(dir, assets),
        None => {
            let mut paths = ContentPaths::from_lookup(lookup);
            if assets.is_some() {
                paths.assets = assets;
            }
            paths
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    const FRONT: &str = "---\ntitle: T\ndescription: D\ntags: []\ndate: \"2025-01-01\"\n---\n";

    fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |var| vars.get(var).cloned()
    }

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn content_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let site = fs::read_to_string(crate::site_config::DEFAULT_SITE_CONFIG_PATH).unwrap();
        write(&dir.path().join("site.toml"), &site);
        write(
            &dir.path().join("garden/welcome.md"),
            &format!("{FRONT}[[missing]]"),
        );
        dir
    }

    #[test]
    fn the_defaults_match_the_current_layout() {
        let paths = ContentPaths::from_lookup(env(&[]));

        assert_eq!(paths.garden_path(), "./garden");
        assert_eq!(paths.site_config, PathBuf::from("./site.toml"));
        assert_eq!(paths.assets, None);
        assert_eq!(paths.asset_dirs(), vec![Path::new(STATIC_DIR)]);
    }

    #[test]
    fn a_content_folder_brings_its_garden_config_and_assets() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("assets")).unwrap();
        let root = dir.path().to_str().unwrap();

        let paths = ContentPaths::from_lookup(env(&[("CONTENT_DIR", root)]));
        assert_eq!(paths.garden, dir.path().join("garden"));
        assert_eq!(paths.site_config, dir.path().join("site.toml"));
        assert_eq!(paths.assets, Some(dir.path().join("assets")));
        assert_eq!(
            paths.asset_dirs(),
            vec![dir.path().join("assets").as_path(), Path::new(STATIC_DIR)]
        );

        let overridden = ContentPaths::from_lookup(env(&[
            ("CONTENT_DIR", root),
            ("ASSETS_DIR", "/elsewhere"),
            ("SITE_CONFIG_PATH", "/app/site.toml"),
        ]));
        assert_eq!(overridden.assets, Some(PathBuf::from("/elsewhere")));
        assert_eq!(overridden.site_config, PathBuf::from("/app/site.toml"));
    }

    #[test]
    fn check_arguments_pick_the_content_folder() {
        let image_env = env(&[("SITE_CONFIG_PATH", "/app/site.toml")]);

        let explicit = check_paths(["/content".to_string()], &image_env).unwrap();
        assert_eq!(explicit.site_config, PathBuf::from("/content/site.toml"));

        let with_assets = check_paths(
            [
                "--assets".to_string(),
                "/fonts".to_string(),
                "/c".to_string(),
            ],
            &image_env,
        )
        .unwrap();
        assert_eq!(with_assets.garden, PathBuf::from("/c/garden"));
        assert_eq!(with_assets.assets, Some(PathBuf::from("/fonts")));

        let from_env = check_paths(Vec::<String>::new(), &image_env).unwrap();
        assert_eq!(from_env.site_config, PathBuf::from("/app/site.toml"));

        assert_eq!(
            check_paths(["--assets".to_string()], &image_env).unwrap_err(),
            "--assets needs a folder"
        );
        assert_eq!(
            check_paths(["--fix".to_string()], &image_env).unwrap_err(),
            "unknown option --fix"
        );
        assert_eq!(
            check_paths(["a".to_string(), "b".to_string()], &image_env).unwrap_err(),
            "unexpected argument b"
        );
    }

    #[test]
    fn checking_reports_config_problems_posts_and_warnings() {
        let dir = content_repo();
        let paths = ContentPaths::new(dir.path(), None);

        let report = check_content(&paths);
        assert!(report.passed(), "{}", report.render());
        assert_eq!(report.posts, 1);
        assert_eq!(
            report.render(),
            "warning: welcome.md: [[missing]] does not match any published post\nChecked 1 posts: 0 problems, 1 warnings\n"
        );

        write(&dir.path().join("garden/broken.md"), "# no front matter");
        let site = fs::read_to_string(dir.path().join("site.toml")).unwrap();
        write(
            &dir.path().join("site.toml"),
            &site.replace("[theme]\nradius = 0", "[theme]\nradius = 99"),
        );

        let report = check_content(&paths);
        assert!(!report.passed());
        assert_eq!(report.problems.len(), 2, "{:?}", report.problems);
        assert!(
            report.problems[0]
                .ends_with("site.toml: theme.radius must be between 0 and 24 pixels, got 99")
        );
        assert!(report.problems[1].starts_with("broken.md: the front matter is missing"));
        assert!(report.render().starts_with("error: "));
    }

    #[test]
    fn unreadable_configs_and_missing_gardens_are_problems() {
        let dir = tempfile::tempdir().unwrap();
        let report = check_content(&ContentPaths::new(dir.path(), None));

        assert_eq!(report.problems.len(), 2);
        assert!(report.problems[0].contains("site.toml: could not read"));
        assert!(report.problems[1].ends_with("garden is not a folder"));
    }
}
