use crate::config::Config;
use crate::content::{parse_published, PostKind};
use crate::{feed, render, scanner, sitemap};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn run(project_root: &Path) -> Result<()> {
    println!("loading config.toml");
    let config = Config::load(&project_root.join("config.toml"))
        .with_context(|| "loading config.toml")?;
    let templates_dir = project_root.join("templates");
    let content_dir = project_root.join("content");
    let static_dir = project_root.join("static");
    let public_dir = project_root.join("public");

    if public_dir.exists() {
        println!("clearing public/");
        fs::remove_dir_all(&public_dir)?;
    }
    fs::create_dir_all(&public_dir)?;

    println!("loading templates");
    let renderer = render::Renderer::new(&templates_dir)?;
    let style_css = fs::read_to_string(static_dir.join("style.css")).ok();
    println!("scanning content/");
    let posts = scanner::scan(&content_dir)?;
    let site = render::Site {
        config: &config,
        style_css: style_css.as_deref(),
    };

    let mut articles: Vec<&_> = posts
        .iter()
        .filter(|p| p.kind == PostKind::Article)
        .collect();
    // datetime desc 정렬. 파싱 실패한 글은 끝(가장 오래된)으로.
    articles.sort_by(|a, b| {
        let pa = parse_published(&a.frontmatter.date).ok();
        let pb = parse_published(&b.frontmatter.date).ok();
        pb.cmp(&pa)
    });
    println!(
        "  found {} article(s), {} page(s)",
        articles.len(),
        posts.len() - articles.len()
    );

    println!("rendering index");
    let index_html = render::render_index(&renderer, &site, &articles)?;
    fs::write(public_dir.join("index.html"), index_html)?;

    for post in &articles {
        println!("rendering post: {}", post.slug);
        let dir = public_dir.join("posts").join(&post.slug);
        fs::create_dir_all(&dir)?;
        let html = render::render_post(&renderer, &site, post)?;
        fs::write(dir.join("index.html"), html)?;
    }

    if let Some(about) = posts
        .iter()
        .find(|p| p.kind == PostKind::Page && p.slug == "about")
    {
        println!("rendering about");
        let dir = public_dir.join("about");
        fs::create_dir_all(&dir)?;
        let html = render::render_about(&renderer, &site, about)?;
        fs::write(dir.join("index.html"), html)?;
    }

    let base = config.base_url.trim_end_matches('/');
    let mut urls = vec![sitemap::SitemapUrl {
        loc: format!("{}/", base),
        lastmod: None,
    }];
    for p in &articles {
        let lastmod = parse_published(&p.frontmatter.date)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|_| p.frontmatter.date.clone());
        urls.push(sitemap::SitemapUrl {
            loc: format!("{}/posts/{}/", base, p.slug),
            lastmod: Some(lastmod),
        });
    }
    if posts
        .iter()
        .any(|p| p.kind == PostKind::Page && p.slug == "about")
    {
        urls.push(sitemap::SitemapUrl {
            loc: format!("{}/about/", base),
            lastmod: None,
        });
    }
    println!("writing sitemap.xml ({} urls)", urls.len());
    fs::write(public_dir.join("sitemap.xml"), sitemap::build_sitemap(&urls))?;
    println!("writing robots.txt");
    fs::write(
        public_dir.join("robots.txt"),
        sitemap::build_robots(&config.base_url),
    )?;
    println!("writing rss.xml ({} items)", articles.len().min(20));
    fs::write(public_dir.join("rss.xml"), feed::build_rss(&config, &articles))?;

    if static_dir.exists() {
        println!("copying static/");
        copy_dir_recursive(&static_dir, &public_dir)?;
    }

    if let Some(host) = custom_domain_host(&config.base_url) {
        println!("writing CNAME ({})", host);
        fs::write(public_dir.join("CNAME"), &host)?;
    }

    Ok(())
}

/// base_url에서 커스텀 도메인 호스트를 뽑는다.
/// `*.github.io` 호스트는 GitHub 기본이므로 CNAME이 필요 없음 → None.
fn custom_domain_host(base_url: &str) -> Option<String> {
    let after_scheme = base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host = after_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if host.is_empty() || host.ends_with("github.io") {
        None
    } else {
        Some(host)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            fs::create_dir_all(&dest)?;
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::custom_domain_host;

    #[test]
    fn extracts_custom_domain() {
        assert_eq!(
            custom_domain_host("https://chiho.one/"),
            Some("chiho.one".into())
        );
        assert_eq!(
            custom_domain_host("https://chiho.one"),
            Some("chiho.one".into())
        );
    }

    #[test]
    fn ignores_github_io_host() {
        assert_eq!(custom_domain_host("https://hyeyoom.github.io/"), None);
        assert_eq!(custom_domain_host("https://hyeyoom.github.io/sub/"), None);
    }

    #[test]
    fn ignores_empty_or_invalid() {
        assert_eq!(custom_domain_host(""), None);
        assert_eq!(custom_domain_host("https://"), None);
    }
}
