use std::fs;
use std::path::Path;

use regex::Regex;
use walkdir::WalkDir;

use crate::links::{LinkIndex, WikiLink, find_wikilinks, markdown_link_target};
use crate::utils::{DEFAULT_LANG, parse_frontmatter, split_translation};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GardenReport {
    pub posts: usize,
    pub problems: Vec<String>,
    pub warnings: Vec<String>,
}

struct CheckedPost {
    source_path: String,
    content: String,
}

pub fn check_garden(garden: &Path) -> GardenReport {
    let mut report = GardenReport::default();
    if !garden.is_dir() {
        report
            .problems
            .push(format!("{} is not a folder", garden.display()));
        return report;
    }

    let mut posts = Vec::new();
    let mut published = Vec::new();
    let is_hidden = |entry: &walkdir::DirEntry| {
        entry.depth() > 0 && entry.file_name().to_string_lossy().starts_with('.')
    };
    let mut entries: Vec<_> = WalkDir::new(garden)
        .into_iter()
        .filter_entry(|entry| !is_hidden(entry))
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "md")
        })
        .collect();
    entries.sort_by(|a, b| a.path().cmp(b.path()));

    for entry in entries {
        let Ok(relative) = entry.path().strip_prefix(garden) else {
            continue;
        };
        let source_path = relative.to_string_lossy().replace('\\', "/");
        let content = match fs::read_to_string(entry.path()) {
            Ok(content) => content,
            Err(e) => {
                report
                    .problems
                    .push(format!("{source_path}: could not be read: {e}"));
                continue;
            }
        };
        if content.trim().is_empty() {
            report.warnings.push(format!(
                "{source_path}: the file is empty, so it is not shown"
            ));
            continue;
        }
        match parse_frontmatter(&content) {
            Ok(metadata) => {
                let (file_path, lang) = split_translation(&source_path);
                if !metadata.is_draft && lang == DEFAULT_LANG {
                    published.push(file_path);
                }
                report.posts += 1;
                posts.push(CheckedPost {
                    source_path,
                    content,
                });
            }
            Err(e) => report.problems.push(format!(
                "{source_path}: the front matter is missing or invalid, so the post is not shown ({e})"
            )),
        }
    }

    let index = LinkIndex::new(&published);
    for post in &posts {
        report.warnings.extend(broken_links(post, &index));
    }
    report
}

fn broken_links(post: &CheckedPost, index: &LinkIndex) -> Vec<String> {
    let markdown_link = Regex::new(r"\[[^\]]+\]\(([^\)\s]+)[^\)]*\)").unwrap();
    let mut warnings = Vec::new();

    for found in find_wikilinks(&post.content) {
        let link = WikiLink::parse(found.inner);
        if !link.target.trim().is_empty() && index.wikilink_target(&link).is_none() {
            warnings.push(format!(
                "{}: [[{}]] does not match any published post",
                post.source_path, found.inner
            ));
        }
    }

    for capture in markdown_link.captures_iter(&post.content) {
        let href = &capture[1];
        let Some(target) = markdown_link_target(href) else {
            continue;
        };
        let file = target.path.rsplit('/').next().unwrap_or(&target.path);
        if file.contains('.') || target.path.is_empty() {
            continue;
        }
        if !index.contains(&target.path) {
            warnings.push(format!(
                "{}: the link to {href} does not match any published post",
                post.source_path
            ));
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRONT: &str = "---\ntitle: T\ndescription: D\ntags: []\ndate: \"2025-01-01\"\n---\n";

    fn garden(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        dir
    }

    #[test]
    fn a_healthy_garden_has_nothing_to_report() {
        let dir = garden(&[
            (
                "welcome.md",
                &format!("{FRONT}See [[fog]], [[fog#Mist|the fog]] and [[#Top]]."),
            ),
            (
                "notes/fog.md",
                &format!("{FRONT}Back to [home](welcome) or [here](../welcome.md#x)."),
            ),
            ("notes/fog.es.md", &format!("{FRONT}Niebla")),
            (".obsidian/broken.md", "no front matter"),
            ("notes/readme.txt", "not a post"),
        ]);

        let report = check_garden(dir.path());

        assert_eq!(report.posts, 3);
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(
            report.warnings,
            vec!["notes/fog.md: the link to ../welcome.md#x does not match any published post"]
        );
    }

    #[test]
    fn posts_without_front_matter_are_problems() {
        let dir = garden(&[
            ("plain.md", "# Just markdown"),
            (
                "no_title.md",
                "---\ndescription: D\ntags: []\ndate: x\n---\n",
            ),
            ("ok.md", FRONT),
            ("placeholder.md", " \n"),
        ]);

        let report = check_garden(dir.path());

        assert_eq!(
            report.warnings,
            vec!["placeholder.md: the file is empty, so it is not shown"]
        );
        assert_eq!(report.posts, 1);
        assert_eq!(report.problems.len(), 2);
        assert!(
            report.problems[0].starts_with("no_title.md: the front matter is missing or invalid")
        );
        assert!(report.problems[1].starts_with("plain.md: the front matter is missing or invalid"));
        assert!(report.problems[1].contains("it has no front matter"));
    }

    #[test]
    fn links_to_missing_or_draft_posts_are_warnings() {
        let draft = FRONT.replace("---\ntitle", "---\nis_draft: true\ntitle");
        let dir = garden(&[
            ("draft.md", &draft),
            (
                "post.md",
                &format!(
                    "{FRONT}[[draft]] [[nowhere]] [gone](notes/gone) [site](https://x.dev) ![pic](img/pic.png) [top](#top)"
                ),
            ),
        ]);

        let report = check_garden(dir.path());

        assert_eq!(
            report.warnings,
            vec![
                "post.md: [[draft]] does not match any published post",
                "post.md: [[nowhere]] does not match any published post",
                "post.md: the link to notes/gone does not match any published post",
            ]
        );
    }

    #[test]
    fn a_missing_garden_is_a_problem() {
        let report = check_garden(Path::new("/definitely/not/a/garden"));
        assert_eq!(
            report.problems,
            vec!["/definitely/not/a/garden is not a folder"]
        );
        assert_eq!(report.posts, 0);
    }
}
