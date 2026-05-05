use crate::content::{slug_from_filename, split_frontmatter, Post, PostKind};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn scan(content_dir: &Path) -> Result<Vec<Post>> {
    let posts_root = content_dir.join("posts");
    let mut posts = Vec::new();
    for entry in WalkDir::new(content_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        let (fm, body) = split_frontmatter(&raw)
            .with_context(|| format!("frontmatter in {}", path.display()))?;
        let stem = path.file_stem().unwrap().to_string_lossy();
        let slug = slug_from_filename(&stem);
        let kind = if path.starts_with(&posts_root) {
            PostKind::Article
        } else {
            PostKind::Page
        };
        posts.push(Post {
            slug,
            frontmatter: fm,
            body_md: body,
            kind,
        });
    }
    Ok(posts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scans_post_and_page() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("posts")).unwrap();
        fs::write(
            dir.path().join("posts/2026-05-05-hello.md"),
            "+++\ntitle = \"Hello\"\ndate = \"2026-05-05\"\n+++\nbody",
        )
        .unwrap();
        fs::write(
            dir.path().join("about.md"),
            "+++\ntitle = \"About\"\ndate = \"2026-01-01\"\n+++\nabout body",
        )
        .unwrap();

        let posts = scan(dir.path()).unwrap();
        assert_eq!(posts.len(), 2);
        let about = posts.iter().find(|p| p.slug == "about").unwrap();
        assert_eq!(about.kind, crate::content::PostKind::Page);
        let hello = posts.iter().find(|p| p.slug == "hello").unwrap();
        assert_eq!(hello.kind, crate::content::PostKind::Article);
        assert_eq!(hello.frontmatter.title, "Hello");
    }

    #[test]
    fn skips_non_markdown_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("note.txt"), "x").unwrap();
        let posts = scan(dir.path()).unwrap();
        assert!(posts.is_empty());
    }
}
