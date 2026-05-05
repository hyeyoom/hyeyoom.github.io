use crate::config::Config;
use crate::content::{parse_published, Post};
use crate::sitemap::xml_escape;

pub fn build_rss(cfg: &Config, articles: &[&Post]) -> String {
    let base = cfg.base_url.trim_end_matches('/');
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<rss version=\"2.0\">\n<channel>\n");
    s.push_str(&format!("<title>{}</title>\n", xml_escape(&cfg.title)));
    s.push_str(&format!("<link>{}/</link>\n", xml_escape(base)));
    s.push_str(&format!(
        "<description>{}</description>\n",
        xml_escape(&cfg.description)
    ));
    s.push_str(&format!(
        "<language>{}</language>\n",
        xml_escape(&cfg.language)
    ));
    for p in articles.iter().take(20) {
        let url = format!("{}/posts/{}/", base, p.slug);
        let pub_date = parse_published(&p.frontmatter.date)
            .map(|dt| dt.to_rfc2822())
            .unwrap_or_else(|_| p.frontmatter.date.clone());
        s.push_str("<item>\n");
        s.push_str(&format!(
            "<title>{}</title>\n",
            xml_escape(&p.frontmatter.title)
        ));
        s.push_str(&format!("<link>{}</link>\n", xml_escape(&url)));
        s.push_str(&format!(
            "<guid isPermaLink=\"true\">{}</guid>\n",
            xml_escape(&url)
        ));
        s.push_str(&format!("<pubDate>{}</pubDate>\n", xml_escape(&pub_date)));
        if let Some(d) = &p.frontmatter.description {
            s.push_str(&format!("<description>{}</description>\n", xml_escape(d)));
        }
        s.push_str("</item>\n");
    }
    s.push_str("</channel>\n</rss>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Frontmatter, PostKind};

    fn cfg() -> Config {
        Config {
            title: "Site".into(),
            author: "Me".into(),
            description: "d".into(),
            base_url: "https://x.com".into(),
            language: "ko".into(),
        }
    }

    fn post(slug: &str, title: &str, date: &str) -> Post {
        Post {
            slug: slug.into(),
            frontmatter: Frontmatter {
                title: title.into(),
                date: date.into(),
                description: Some("d".into()),
                math: false,
            },
            body_md: String::new(),
            kind: PostKind::Article,
        }
    }

    #[test]
    fn rss_has_channel_and_items() {
        let p1 = post("a", "First", "2026-01-01");
        let p2 = post("b", "Second", "2026-02-01");
        let xml = build_rss(&cfg(), &[&p2, &p1]);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<rss"));
        assert!(xml.contains("<channel>"));
        assert!(xml.contains("<title>Site</title>"));
        assert!(xml.contains("<link>https://x.com/</link>"));
        assert!(xml.contains("<item>"));
        assert!(xml.contains("https://x.com/posts/a/"));
        assert!(xml.contains("https://x.com/posts/b/"));
    }
}
