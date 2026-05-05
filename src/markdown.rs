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
