use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
use std::sync::OnceLock;
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

pub fn render(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(md, opts);

    let mut events: Vec<Event> = Vec::new();
    let mut in_code_lang: Option<String> = None;
    let mut code_buf = String::new();

    for ev in parser {
        match ev {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                in_code_lang = Some(lang.into_string());
                code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                let lang = in_code_lang.take().unwrap_or_default();
                let html = highlight_code(&code_buf, &lang);
                events.push(Event::Html(CowStr::Boxed(html.into_boxed_str())));
                code_buf.clear();
            }
            Event::Text(t) if in_code_lang.is_some() => {
                code_buf.push_str(&t);
            }
            other => events.push(other),
        }
    }

    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    enhance_footnotes(&out)
}

fn enhance_footnotes(html: &str) -> String {
    let needle = "<sup class=\"footnote-reference\"><a href=\"#";
    let close = "</a></sup>";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    let mut refs: Vec<(String, String, String, usize)> = Vec::new();

    while let Some(start) = rest.find(needle) {
        out.push_str(&rest[..start]);

        let href_start = start + needle.len();
        let Some(href_end_rel) = rest[href_start..].find("\">") else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let href_end = href_start + href_end_rel;
        let footnote_id = &rest[href_start..href_end];
        let label_start = href_end + 2;
        let Some(label_end_rel) = rest[label_start..].find(close) else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let label_end = label_start + label_end_rel;
        let label = &rest[label_start..label_end];
        let after_ref = label_end + close.len();
        let ordinal = refs
            .iter()
            .filter(|(seen_id, _, _, _)| seen_id == footnote_id)
            .count()
            + 1;
        let ref_id = if ordinal == 1 {
            format!("fnref-{}", footnote_id)
        } else {
            format!("fnref-{}-{}", footnote_id, ordinal)
        };
        refs.push((
            footnote_id.to_string(),
            label.to_string(),
            ref_id.clone(),
            ordinal,
        ));
        out.push_str(&format!(
            r##"<sup class="footnote-reference" id="{}"><a href="#{}" aria-label="각주 {} 보기">[{}]</a></sup>"##,
            ref_id, footnote_id, label, label
        ));

        rest = &rest[after_ref..];
    }

    out.push_str(rest);

    let mut linked_definitions: Vec<String> = Vec::new();
    for (footnote_id, label, ref_id, _) in &refs {
        if linked_definitions.iter().any(|id| id == footnote_id) {
            continue;
        }
        linked_definitions.push(footnote_id.clone());

        let definition = format!(r#"<div class="footnote-definition" id="{}">"#, footnote_id);
        if out.contains(&definition) {
            let old_label = format!(
                r#"<div class="footnote-definition" id="{}"><sup class="footnote-definition-label">{}</sup>"#,
                footnote_id, label
            );
            let new_label = format!(
                r##"<div class="footnote-definition" id="{}"><sup class="footnote-definition-label"><a href="#fnref-{}" aria-label="본문 각주 {}로 돌아가기">{}</a></sup>"##,
                footnote_id, footnote_id, label, label
            );
            out = out.replacen(&old_label, &new_label, 1);

            let related_refs: Vec<_> = refs
                .iter()
                .filter(|(id, _, _, _)| id == footnote_id)
                .collect();
            let mut backlink = format!(
                r##" <span class="footnote-backrefs"><a class="footnote-backref" href="#{}" aria-label="본문 각주 {}로 돌아가기">본문으로 돌아가기 ↑</a>"##,
                ref_id, label
            );
            for (_, repeated_label, repeated_ref_id, ordinal) in related_refs.iter().skip(1) {
                backlink.push_str(&format!(
                    r##" <a class="footnote-backref footnote-backref-extra" href="#{}" aria-label="본문 각주 {}의 {}번째 참조로 돌아가기">↑{}</a>"##,
                    repeated_ref_id, repeated_label, ordinal, ordinal
                ));
            }
            backlink.push_str("</span>");
            let search_from = out.find(&definition).unwrap_or(0);
            if let Some(close_rel) = out[search_from..].find("</p>") {
                let close = search_from + close_rel;
                out.insert_str(close, &backlink);
            }
        }
    }

    out
}

fn highlight_code(code: &str, lang: &str) -> String {
    let ss = syntax_set();
    let ts = theme_set();
    let theme = &ts.themes["InspiredGitHub"];
    let syntax = ss
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| ss.find_syntax_plain_text());
    highlighted_html_for_string(code, ss, syntax, theme)
        .unwrap_or_else(|_| format!("<pre><code>{}</code></pre>", html_escape(code)))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_paragraph_with_bold() {
        let html = render("Hello **world**");
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn renders_footnote_definition() {
        let html = render("Note[^1].\n\n[^1]: Footnote text");
        assert!(html.contains("footnote-definition"));
    }

    #[test]
    fn footnotes_link_both_ways() {
        let html = render("Note[^note].\n\n[^note]: Footnote text");
        assert!(html.contains(r##"id="fnref-note""##));
        assert!(html.contains(r##"href="#note""##));
        assert!(html.contains(r##"href="#fnref-note""##));
        assert!(html.contains(r##"footnote-definition-label"><a href="#fnref-note""##));
        assert!(html.contains("[1]"));
        assert!(html.contains("본문으로 돌아가기"));
    }

    #[test]
    fn repeated_footnote_refs_have_one_backlink() {
        let html = render("One[^note]. Two[^note].\n\n[^note]: Footnote text");

        assert_eq!(html.matches("본문으로 돌아가기").count(), 1);
        assert_eq!(html.matches(r##"id="fnref-note""##).count(), 1);
        assert_eq!(html.matches(r##"id="fnref-note-2""##).count(), 1);
        assert_eq!(html.matches(r##"href="#note" aria-label="각주 1 보기">[1]"##).count(), 2);
        assert!(html.contains(r##"href="#fnref-note-2""##));
        assert!(html.contains(">↑2</a>"));
    }

    #[test]
    fn passes_inline_math_through() {
        let html = render("Inline $a^2 + b^2$ here");
        assert!(html.contains("$a^2 + b^2$"));
    }

    #[test]
    fn passes_display_math_through() {
        let html = render("$$\\int x dx$$");
        assert!(html.contains("$$"));
        assert!(html.contains("\\int"));
    }

    #[test]
    fn renders_image_tag() {
        let html = render("![alt text](/foo.png)");
        assert!(html.contains("<img"));
        assert!(html.contains("/foo.png"));
        assert!(html.contains("alt text"));
    }

    #[test]
    fn highlights_known_language_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let html = render(md);
        assert!(html.contains("<pre"));
        assert!(html.contains("style=\""), "syntect should emit inline style");
        assert!(html.contains("fn"));
        assert!(html.contains("main"));
    }

    #[test]
    fn renders_unknown_language_as_plain_pre() {
        let md = "```xyzlang\nhello world\n```";
        let html = render(md);
        assert!(html.contains("<pre"));
        assert!(html.contains("hello world"));
    }
}
