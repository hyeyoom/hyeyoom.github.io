pub struct SitemapUrl {
    pub loc: String,
    pub lastmod: Option<String>,
}

pub fn build_sitemap(urls: &[SitemapUrl]) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for u in urls {
        s.push_str("  <url>\n");
        s.push_str(&format!("    <loc>{}</loc>\n", xml_escape(&u.loc)));
        if let Some(lm) = &u.lastmod {
            s.push_str(&format!("    <lastmod>{}</lastmod>\n", xml_escape(lm)));
        }
        s.push_str("  </url>\n");
    }
    s.push_str("</urlset>\n");
    s
}

pub fn build_robots(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    format!("User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n", base)
}

pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_sitemap_xml_with_loc_and_lastmod() {
        let urls = vec![
            SitemapUrl {
                loc: "https://x.com/".into(),
                lastmod: None,
            },
            SitemapUrl {
                loc: "https://x.com/posts/a/".into(),
                lastmod: Some("2026-05-05".into()),
            },
        ];
        let xml = build_sitemap(&urls);
        assert!(xml.starts_with("<?xml"));
        assert!(xml.contains("<urlset"));
        assert!(xml.contains("<loc>https://x.com/posts/a/</loc>"));
        assert!(xml.contains("<lastmod>2026-05-05</lastmod>"));
        assert!(xml.contains("</urlset>"));
    }

    #[test]
    fn generates_robots_txt_with_sitemap() {
        let r = build_robots("https://x.com/");
        assert!(r.contains("User-agent: *"));
        assert!(r.contains("Allow: /"));
        assert!(r.contains("Sitemap: https://x.com/sitemap.xml"));
    }

    #[test]
    fn xml_escapes_special_chars() {
        let urls = vec![SitemapUrl {
            loc: "https://x.com/?a=1&b=2".into(),
            lastmod: None,
        }];
        let xml = build_sitemap(&urls);
        assert!(xml.contains("&amp;"));
        assert!(!xml.contains("?a=1&b=2"));
    }
}
