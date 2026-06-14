use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct FixtureCleanup {
    files: Vec<PathBuf>,
}

impl Drop for FixtureCleanup {
    fn drop(&mut self) {
        for file in &self.files {
            let _ = fs::remove_file(file);
        }
    }
}

#[test]
fn cargo_run_build_produces_complete_site() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let translation_fixture =
        root.join("content/translations/2026-06-14-integration-translation.md");
    let draft_fixture = root.join("content/drafts/blog/2026-06-14-integration-draft.md");
    fs::create_dir_all(translation_fixture.parent().unwrap()).unwrap();
    fs::create_dir_all(draft_fixture.parent().unwrap()).unwrap();
    fs::write(
        &translation_fixture,
        "+++\ntitle = \"Integration Translation\"\ndate = \"2026-06-14\"\n+++\ntranslated body",
    )
    .unwrap();
    fs::write(
        &draft_fixture,
        "+++\ntitle = \"Integration Draft\"\ndate = \"2026-06-14\"\n+++\ndraft body",
    )
    .unwrap();
    let _cleanup = FixtureCleanup {
        files: vec![translation_fixture, draft_fixture],
    };

    let public = root.join("public");
    if public.exists() {
        fs::remove_dir_all(&public).unwrap();
    }

    let status = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--", "build"])
        .current_dir(root)
        .status()
        .expect("cargo run failed");
    assert!(status.success(), "build did not succeed");

    let index = fs::read_to_string(public.join("index.html")).unwrap();
    assert!(index.contains("Hello, World"));
    assert!(index.contains("/posts/hello-world/"));
    assert!(index.contains("Integration Translation"));
    assert!(index.contains("/translations/integration-translation/"));
    assert!(!index.contains("Integration Draft"));
    assert!(index.contains("og:type"));
    assert!(index.contains("href=\"/translations/\""));
    assert!(index.contains("href=\"/search/\""));
    assert!(!index.contains("site-search-input"));
    assert!(!index.contains("/search.js"));

    let search_page = fs::read_to_string(public.join("search/index.html")).unwrap();
    assert!(search_page.contains("SEARCH"));
    assert!(search_page.contains("site-search-input"));
    assert!(search_page.contains("/search.js"));

    let search_index = fs::read_to_string(public.join("search-index.json")).unwrap();
    assert!(search_index.contains("Hello, World"));
    assert!(search_index.contains("/posts/hello-world/"));
    assert!(search_index.contains("Integration Translation"));
    assert!(search_index.contains("/translations/integration-translation/"));
    assert!(!search_index.contains("Integration Draft"));
    assert!(search_index.contains("keywords"));

    let search_js = fs::read_to_string(public.join("search.js")).unwrap();
    assert!(search_js.contains("search-index.json"));
    assert!(search_js.contains("site-search-input"));

    let post = fs::read_to_string(public.join("posts/hello-world/index.html")).unwrap();
    assert!(post.contains("Hello, World"));
    assert!(post.contains("application/ld+json"));
    assert!(post.contains("\"@type\":\"Article\""));
    assert!(post.contains("katex"));
    assert!(
        post.contains("$E = mc^2$"),
        "math should pass through to client"
    );
    assert!(post.contains("footnote-definition"));
    assert!(post.contains("/images/placeholder.svg"));
    assert!(post.contains("<pre"));
    assert!(post.contains("println"));
    assert!(
        post.contains("style=\""),
        "syntect inline style should be present"
    );

    let translation =
        fs::read_to_string(public.join("translations/integration-translation/index.html")).unwrap();
    assert!(translation.contains("Integration Translation"));
    assert!(translation.contains("https://chiho.one/translations/integration-translation/"));

    let translations_index = fs::read_to_string(public.join("translations/index.html")).unwrap();
    assert!(translations_index.contains("TRANSLATIONS"));
    assert!(translations_index.contains("Integration Translation"));
    assert!(translations_index.contains("/translations/integration-translation/"));
    assert!(!translations_index.contains("Integration Draft"));

    let about = fs::read_to_string(public.join("about/index.html")).unwrap();
    assert!(about.contains("ABOUT"));

    let sitemap = fs::read_to_string(public.join("sitemap.xml")).unwrap();
    assert!(sitemap.contains("hello-world"));
    assert!(sitemap.contains("/translations/integration-translation/"));
    assert!(!sitemap.contains("integration-draft"));
    assert!(sitemap.contains("/about/"));

    let robots = fs::read_to_string(public.join("robots.txt")).unwrap();
    assert!(robots.contains("Sitemap"));

    let rss = fs::read_to_string(public.join("rss.xml")).unwrap();
    assert!(rss.contains("Hello, World"));
    assert!(!rss.contains("Integration Translation"));
    assert!(rss.contains("<channel>"));

    assert!(
        public.join("style.css").exists(),
        "static/style.css should be copied"
    );
    assert!(public.join("images/placeholder.svg").exists());
}
