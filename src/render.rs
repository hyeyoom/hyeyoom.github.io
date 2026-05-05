use crate::config::Config;
use crate::content::{display_kst, parse_published, Post};
use crate::markdown;
use anyhow::Result;
use serde_json::json;
use std::path::Path;
use tera::{Context, Tera};

fn post_dates(raw: &str) -> (String, String) {
    match parse_published(raw) {
        Ok(dt) => (display_kst(&dt), dt.to_rfc3339()),
        Err(_) => (raw.to_string(), raw.to_string()),
    }
}

pub struct Renderer {
    tera: Tera,
}

impl Renderer {
    pub fn new(templates_dir: &Path) -> Result<Self> {
        let pattern = format!("{}/**/*.html", templates_dir.display());
        let tera = Tera::new(&pattern)?;
        Ok(Self { tera })
    }

    pub fn render(&self, name: &str, ctx: &Context) -> Result<String> {
        Ok(self.tera.render(name, ctx)?)
    }
}

pub struct Site<'a> {
    pub config: &'a Config,
}

fn site_value(cfg: &Config) -> serde_json::Value {
    let google_analytics_id = cfg
        .google_analytics_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());

    json!({
        "title": cfg.title,
        "language": cfg.language,
        "author": cfg.author,
        "description": cfg.description,
        "google_analytics_id": google_analytics_id,
    })
}

pub fn render_post(renderer: &Renderer, site: &Site, post: &Post) -> Result<String> {
    let body_html = markdown::render(&post.body_md);
    let base = site.config.base_url.trim_end_matches('/');
    let canonical = format!("{}/posts/{}/", base, post.slug);
    let description = post
        .frontmatter
        .description
        .clone()
        .unwrap_or_else(|| site.config.description.clone());
    let (display_date, iso_date) = post_dates(&post.frontmatter.date);
    let json_ld = json!({
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": post.frontmatter.title,
        "datePublished": iso_date,
        "author": { "@type": "Person", "name": site.config.author },
        "mainEntityOfPage": canonical,
    })
    .to_string();

    let mut ctx = Context::new();
    ctx.insert("site", &site_value(site.config));
    ctx.insert(
        "page_title",
        &format!("{} — {}", post.frontmatter.title, site.config.title),
    );
    ctx.insert("description", &description);
    ctx.insert("canonical", &canonical);
    ctx.insert("og_type", "article");
    ctx.insert("math", &post.frontmatter.math);
    ctx.insert("json_ld", &json_ld);
    ctx.insert(
        "post",
        &json!({
            "title": post.frontmatter.title,
            "date": display_date,
            "body": body_html,
        }),
    );
    renderer.render("post.html", &ctx)
}

pub fn render_index(renderer: &Renderer, site: &Site, articles: &[&Post]) -> Result<String> {
    let base = site.config.base_url.trim_end_matches('/');
    let canonical = format!("{}/", base);
    let posts: Vec<_> = articles
        .iter()
        .map(|p| {
            let (display_date, _) = post_dates(&p.frontmatter.date);
            json!({
                "title": p.frontmatter.title,
                "date": display_date,
                "url": format!("/posts/{}/", p.slug),
            })
        })
        .collect();

    let mut ctx = Context::new();
    ctx.insert("site", &site_value(site.config));
    ctx.insert("page_title", &site.config.title);
    ctx.insert("description", &site.config.description);
    ctx.insert("canonical", &canonical);
    ctx.insert("og_type", "website");
    ctx.insert("math", &false);
    ctx.insert("posts", &posts);
    renderer.render("index.html", &ctx)
}

pub fn render_about(renderer: &Renderer, site: &Site, page: &Post) -> Result<String> {
    let body_html = markdown::render(&page.body_md);
    let base = site.config.base_url.trim_end_matches('/');
    let canonical = format!("{}/about/", base);
    let description = page
        .frontmatter
        .description
        .clone()
        .unwrap_or_else(|| site.config.description.clone());

    let mut ctx = Context::new();
    ctx.insert("site", &site_value(site.config));
    ctx.insert(
        "page_title",
        &format!("About — {}", site.config.title),
    );
    ctx.insert("description", &description);
    ctx.insert("canonical", &canonical);
    ctx.insert("og_type", "website");
    ctx.insert("math", &page.frontmatter.math);
    ctx.insert("body", &body_html);
    renderer.render("about.html", &ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Frontmatter, PostKind};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn loads_and_renders_template() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("hello.html"), "<p>Hi {{ name }}</p>").unwrap();
        let r = Renderer::new(dir.path()).unwrap();
        let mut ctx = tera::Context::new();
        ctx.insert("name", "world");
        let out = r.render("hello.html", &ctx).unwrap();
        assert_eq!(out, "<p>Hi world</p>");
    }

    fn test_config() -> Config {
        Config {
            title: "S".into(),
            author: "A".into(),
            description: "D".into(),
            base_url: "https://example.com".into(),
            language: "ko".into(),
            google_analytics_id: None,
        }
    }

    fn test_post(math: bool) -> Post {
        Post {
            slug: "hello".into(),
            frontmatter: Frontmatter {
                title: "Hello".into(),
                date: "2026-05-05".into(),
                description: Some("desc".into()),
                math,
            },
            body_md: "Body **bold**".into(),
            kind: PostKind::Article,
        }
    }

    fn project_templates() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates")
    }

    #[test]
    fn post_html_includes_seo_metadata() {
        let renderer = Renderer::new(&project_templates()).unwrap();
        let cfg = test_config();
        let site = Site { config: &cfg };
        let html = render_post(&renderer, &site, &test_post(false)).unwrap();
        assert!(html.contains("<title>Hello — S</title>"));
        assert!(html.contains("og:title"));
        assert!(html.contains("og:type\" content=\"article\""));
        assert!(html.contains("https://example.com/posts/hello/"));
        assert!(html.contains("application/ld+json"));
        assert!(html.contains("\"@type\":\"Article\""));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn post_html_includes_katex_when_math_true() {
        let renderer = Renderer::new(&project_templates()).unwrap();
        let cfg = test_config();
        let site = Site { config: &cfg };
        let html = render_post(&renderer, &site, &test_post(true)).unwrap();
        assert!(html.contains("katex"));
    }

    #[test]
    fn post_html_omits_katex_when_math_false() {
        let renderer = Renderer::new(&project_templates()).unwrap();
        let cfg = test_config();
        let site = Site { config: &cfg };
        let html = render_post(&renderer, &site, &test_post(false)).unwrap();
        assert!(!html.contains("katex"));
    }

    #[test]
    fn post_html_includes_google_analytics_when_configured() {
        let renderer = Renderer::new(&project_templates()).unwrap();
        let mut cfg = test_config();
        cfg.google_analytics_id = Some("G-TEST123".into());
        let site = Site { config: &cfg };
        let html = render_post(&renderer, &site, &test_post(false)).unwrap();
        assert!(html.contains("googletagmanager.com/gtag/js?id=G-TEST123"));
        assert!(html.contains("gtag('config', 'G-TEST123');"));
    }

    #[test]
    fn post_html_omits_google_analytics_when_not_configured() {
        let renderer = Renderer::new(&project_templates()).unwrap();
        let cfg = test_config();
        let site = Site { config: &cfg };
        let html = render_post(&renderer, &site, &test_post(false)).unwrap();
        assert!(!html.contains("googletagmanager.com/gtag/js"));
    }

    #[test]
    fn index_html_lists_articles() {
        let renderer = Renderer::new(&project_templates()).unwrap();
        let cfg = test_config();
        let site = Site { config: &cfg };
        let post = test_post(false);
        let html = render_index(&renderer, &site, &[&post]).unwrap();
        assert!(html.contains("Hello"));
        assert!(html.contains("/posts/hello/"));
        assert!(html.contains("og:type\" content=\"website\""));
    }

    #[test]
    fn about_html_renders_body() {
        let renderer = Renderer::new(&project_templates()).unwrap();
        let cfg = test_config();
        let site = Site { config: &cfg };
        let mut about = test_post(false);
        about.slug = "about".into();
        about.kind = PostKind::Page;
        about.body_md = "About me".into();
        let html = render_about(&renderer, &site, &about).unwrap();
        assert!(html.contains("ABOUT"));
        assert!(html.contains("About me"));
        assert!(html.contains("https://example.com/about/"));
    }
}
