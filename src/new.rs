use crate::content::kst;
use anyhow::Result;
use chrono::{SecondsFormat, Utc};
use slug::slugify;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run(project_root: &Path, title: &str) -> Result<PathBuf> {
    let now = Utc::now().with_timezone(&kst());
    let filename_date = now.format("%Y-%m-%d").to_string();
    let frontmatter_date = now.to_rfc3339_opts(SecondsFormat::Secs, false);
    let slug = slugify(title);
    let filename = format!("{}-{}.md", filename_date, slug);
    let dir = project_root.join("content").join("posts");
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn creates_post_file_with_dated_filename_and_rfc3339_frontmatter() {
        let dir = tempdir().unwrap();
        let path = run(dir.path(), "Hello World").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("title = \"Hello World\""));
        assert!(content.contains("+++"));
        assert!(content.contains("math = false"));
        // KST RFC 3339 (예: 2026-05-05T14:30:00+09:00)
        assert!(content.contains("+09:00"), "frontmatter should embed KST offset");
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
        assert!(path.starts_with(dir.path().join("content/posts")));
    }

    #[test]
    fn escapes_quotes_in_title() {
        let dir = tempdir().unwrap();
        let path = run(dir.path(), "Say \"hi\"").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("title = \"Say \\\"hi\\\"\""));
    }
}
