use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn cargo_run_build_produces_complete_site() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

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
    assert!(index.contains("og:type"));
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
    assert!(search_index.contains("keywords"));

    let search_js = fs::read_to_string(public.join("search.js")).unwrap();
    assert!(search_js.contains("search-index.json"));
    assert!(search_js.contains("site-search-input"));

    let post = fs::read_to_string(public.join("posts/hello-world/index.html")).unwrap();
    assert!(post.contains("Hello, World"));
    assert!(post.contains("application/ld+json"));
    assert!(post.contains("\"@type\":\"Article\""));
    assert!(post.contains("katex"));
    assert!(post.contains("$E = mc^2$"), "math should pass through to client");
    assert!(post.contains("footnote-definition"));
    assert!(post.contains("/images/placeholder.svg"));
    assert!(post.contains("<pre"));
    assert!(post.contains("println"));
    assert!(
        post.contains("style=\""),
        "syntect inline style should be present"
    );

    let about = fs::read_to_string(public.join("about/index.html")).unwrap();
    assert!(about.contains("ABOUT"));

    let sitemap = fs::read_to_string(public.join("sitemap.xml")).unwrap();
    assert!(sitemap.contains("hello-world"));
    assert!(sitemap.contains("/about/"));

    let robots = fs::read_to_string(public.join("robots.txt")).unwrap();
    assert!(robots.contains("Sitemap"));

    let rss = fs::read_to_string(public.join("rss.xml")).unwrap();
    assert!(rss.contains("Hello, World"));
    assert!(rss.contains("<channel>"));

    assert!(
        public.join("style.css").exists(),
        "static/style.css should be copied"
    );
    assert!(public.join("images/placeholder.svg").exists());
}
