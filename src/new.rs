use crate::content::kst;
use anyhow::{anyhow, bail, Result};
use chrono::{SecondsFormat, Utc};
use slug::slugify;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentSection {
    Blog,
    Translation,
}

impl ContentSection {
    fn draft_dir(self) -> &'static str {
        match self {
            ContentSection::Blog => "blog",
            ContentSection::Translation => "translations",
        }
    }

    fn published_dir(self) -> &'static str {
        match self {
            ContentSection::Blog => "posts",
            ContentSection::Translation => "translations",
        }
    }
}

pub fn run(project_root: &Path, title: &str) -> Result<PathBuf> {
    draft(project_root, ContentSection::Blog, title)
}

pub fn draft(project_root: &Path, section: ContentSection, title: &str) -> Result<PathBuf> {
    let now = Utc::now().with_timezone(&kst());
    let filename_date = now.format("%Y-%m-%d").to_string();
    let frontmatter_date = now.to_rfc3339_opts(SecondsFormat::Secs, false);
    let slug = slugify(title);
    let filename = format!("{}-{}.md", filename_date, slug);
    let dir = project_root
        .join("content")
        .join("drafts")
        .join(section.draft_dir());
    fs::create_dir_all(&dir)?;
    let path = dir.join(&filename);
    let escaped = title.replace('"', "\\\"");
    let body = format!(
        "+++\ntitle = \"{}\"\ndate = \"{}\"\nmath = false\n+++\n\n",
        escaped, frontmatter_date,
    );
    fs::write(&path, body)?;
    Ok(path)
}

pub fn publish(project_root: &Path, section: ContentSection, draft_ref: &str) -> Result<PathBuf> {
    let draft_path = resolve_draft(project_root, section, draft_ref)?;
    let filename = draft_path
        .file_name()
        .ok_or_else(|| anyhow!("draft path has no filename: {}", draft_path.display()))?;
    let published_dir = project_root.join("content").join(section.published_dir());
    fs::create_dir_all(&published_dir)?;
    let published_path = published_dir.join(filename);
    if published_path.exists() {
        bail!(
            "published file already exists: {}",
            published_path.display()
        );
    }
    fs::rename(&draft_path, &published_path)?;
    Ok(published_path)
}

fn resolve_draft(project_root: &Path, section: ContentSection, draft_ref: &str) -> Result<PathBuf> {
    let input = Path::new(draft_ref);
    if input.exists() {
        return Ok(input.to_path_buf());
    }

    let draft_dir = project_root
        .join("content")
        .join("drafts")
        .join(section.draft_dir());
    let candidates = [
        draft_dir.join(draft_ref),
        draft_dir.join(format!("{}.md", draft_ref.trim_end_matches(".md"))),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    for entry in fs::read_dir(&draft_dir)
        .map_err(|_| anyhow!("draft not found in {}: {}", draft_dir.display(), draft_ref))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem == draft_ref || stem.ends_with(&format!("-{}", draft_ref)) {
            return Ok(path);
        }
    }

    bail!("draft not found in {}: {}", draft_dir.display(), draft_ref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn creates_blog_draft_with_dated_filename_and_rfc3339_frontmatter() {
        let dir = tempdir().unwrap();
        let path = run(dir.path(), "Hello World").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("title = \"Hello World\""));
        assert!(content.contains("+++"));
        assert!(content.contains("math = false"));
        // KST RFC 3339 (예: 2026-05-05T14:30:00+09:00)
        assert!(
            content.contains("+09:00"),
            "frontmatter should embed KST offset"
        );
        let kst_today = Utc::now()
            .with_timezone(&kst())
            .format("%Y-%m-%d")
            .to_string();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with(&kst_today),
            "filename {} should start with KST date {}",
            name,
            kst_today
        );
        assert!(name.contains("hello-world"));
        assert!(path.starts_with(dir.path().join("content/drafts/blog")));
    }

    #[test]
    fn creates_blog_and_translation_drafts_in_section_directories() {
        let dir = tempdir().unwrap();
        let blog = draft(dir.path(), ContentSection::Blog, "Hello Blog").unwrap();
        let translation =
            draft(dir.path(), ContentSection::Translation, "Hello Translation").unwrap();

        assert!(blog.starts_with(dir.path().join("content/drafts/blog")));
        assert!(translation.starts_with(dir.path().join("content/drafts/translations")));
        assert!(std::fs::read_to_string(blog)
            .unwrap()
            .contains("title = \"Hello Blog\""));
        assert!(std::fs::read_to_string(translation)
            .unwrap()
            .contains("title = \"Hello Translation\""));
    }

    #[test]
    fn publishes_draft_to_section_publish_directory() {
        let dir = tempdir().unwrap();
        let draft_path =
            draft(dir.path(), ContentSection::Translation, "Translated Thing").unwrap();
        let published = publish(
            dir.path(),
            ContentSection::Translation,
            draft_path.to_str().unwrap(),
        )
        .unwrap();

        assert!(published.starts_with(dir.path().join("content/translations")));
        assert!(published.exists());
        assert!(!draft_path.exists());
    }

    #[test]
    fn publish_refuses_to_overwrite_existing_file() {
        let dir = tempdir().unwrap();
        let draft_path = draft(dir.path(), ContentSection::Blog, "Same Title").unwrap();
        let target_dir = dir.path().join("content/posts");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join(draft_path.file_name().unwrap()), "existing").unwrap();

        let err = publish(
            dir.path(),
            ContentSection::Blog,
            draft_path.to_str().unwrap(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn escapes_quotes_in_title() {
        let dir = tempdir().unwrap();
        let path = run(dir.path(), "Say \"hi\"").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("title = \"Say \\\"hi\\\"\""));
    }
}
