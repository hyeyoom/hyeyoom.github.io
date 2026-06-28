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
    let md = cjk_friendly_emphasis(md);

    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(&md, opts);

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

/// Zero-width space (U+200B). It belongs to the Unicode `Cf` (format) category,
/// so CommonMark treats it as an ordinary character — neither whitespace nor
/// punctuation.
const ZWSP: char = '\u{200B}';

/// Returns true for CJK "letters" (Hangul, Han ideographs, Kana). These are the
/// characters that CommonMark classifies as ordinary characters, which is what
/// breaks emphasis when an emphasis delimiter sits between such a character and
/// a punctuation mark.
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{1100}'..='\u{11FF}'   // Hangul Jamo
        | '\u{3040}'..='\u{30FF}' // Hiragana + Katakana
        | '\u{3130}'..='\u{318F}' // Hangul Compatibility Jamo
        | '\u{3400}'..='\u{4DBF}' // CJK Unified Ideographs Extension A
        | '\u{4E00}'..='\u{9FFF}' // CJK Unified Ideographs
        | '\u{A960}'..='\u{A97F}' // Hangul Jamo Extended-A
        | '\u{AC00}'..='\u{D7A3}' // Hangul Syllables
        | '\u{D7B0}'..='\u{D7FF}' // Hangul Jamo Extended-B
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        | '\u{20000}'..='\u{2FA1F}' // CJK Ext B..F + Compatibility Supplement
    )
}

/// Returns true for characters CommonMark treats as punctuation for the purpose
/// of computing left/right-flanking delimiter runs. We cover ASCII punctuation
/// (the common case) plus the CJK fullwidth/wide punctuation that frequently
/// appears next to emphasis in Korean/Japanese/Chinese prose.
fn is_md_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(c,
            '\u{3001}' | '\u{3002}'      // 、 。
            | '\u{3008}'..='\u{3011}'    // 〈〉《》「」『』【】
            | '\u{3014}'..='\u{301B}'    // 〔〕〖〗〘〙〚〛
            | '\u{FF01}' | '\u{FF08}' | '\u{FF09}' | '\u{FF0C}'  // ！（）、，
            | '\u{FF1A}' | '\u{FF1B}' | '\u{FF1F}'               // ：；？
        )
}

/// Works around CommonMark's emphasis flanking rules being unfriendly to CJK
/// text. In CommonMark a `**`/`*`/`__`/`_` delimiter run that sits directly
/// between a punctuation character and a CJK character is neither left- nor
/// right-flanking (CJK letters count as ordinary characters, so the run is
/// "stuck" to the punctuation), which means it cannot open or close emphasis.
///
/// For example `**가상 메모리(Virtual Memory)**라는` fails because the closing
/// `**` is preceded by `)` (punctuation) and followed by `라` (a CJK letter),
/// so it never closes the strong span and the literal `**` leak into the output.
///
/// We fix this by inserting a zero-width space between the delimiter run and the
/// adjacent punctuation. The zero-width space is an ordinary character, so the
/// run now sees an ordinary character on the punctuation side and an ordinary
/// CJK character on the other side, restoring its flanking status. The inserted
/// character is invisible in the rendered output.
///
/// Code (fenced blocks, indented blocks and inline code spans) is left
/// untouched so that source code is never modified.
fn cjk_friendly_emphasis(md: &str) -> String {
    let mut out = String::with_capacity(md.len() + 16);
    let mut fence: Option<(char, usize)> = None;
    let mut first = true;

    for line in md.split('\n') {
        if !first {
            out.push('\n');
        }
        first = false;

        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        // Inside a fenced code block: copy verbatim, watching for the close.
        if let Some((fc, flen)) = fence {
            out.push_str(line);
            let closes = trimmed.chars().take_while(|&c| c == fc).count() >= flen
                && trimmed.chars().all(|c| c == fc || c.is_whitespace());
            if closes {
                fence = None;
            }
            continue;
        }

        // Opening of a fenced code block.
        if indent < 4 && (trimmed.starts_with("```") || trimmed.starts_with("~~~")) {
            let fc = trimmed.chars().next().unwrap();
            let flen = trimmed.chars().take_while(|&c| c == fc).count();
            fence = Some((fc, flen));
            out.push_str(line);
            continue;
        }

        // Indented code block: leave untouched to avoid corrupting source.
        if line.starts_with("    ") || line.starts_with('\t') {
            out.push_str(line);
            continue;
        }

        transform_line(line, &mut out);
    }

    out
}

/// Applies the CJK emphasis fix to a single line, skipping inline code spans.
fn transform_line(line: &str, out: &mut String) {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut text_start = 0;

    while i < n {
        if chars[i] == '`' {
            // Length of this backtick run.
            let mut j = i;
            while j < n && chars[j] == '`' {
                j += 1;
            }
            let run_len = j - i;

            // Find a matching closing backtick run of the same length.
            let mut k = j;
            let mut close: Option<usize> = None;
            while k < n {
                if chars[k] == '`' {
                    let mut m = k;
                    while m < n && chars[m] == '`' {
                        m += 1;
                    }
                    if m - k == run_len {
                        close = Some(m);
                        break;
                    }
                    k = m;
                } else {
                    k += 1;
                }
            }

            if let Some(end) = close {
                // Transform the text before the code span, then copy the code
                // span (delimiters included) verbatim.
                transform_segment(&chars[text_start..i], out);
                out.extend(&chars[i..end]);
                i = end;
                text_start = end;
            } else {
                // No closing run: treat the backticks as literal text.
                i = j;
            }
        } else {
            i += 1;
        }
    }

    transform_segment(&chars[text_start..n], out);
}

/// Inserts zero-width spaces around emphasis delimiter runs that are wedged
/// between a CJK character and a punctuation character within a plain-text
/// (non-code) segment.
fn transform_segment(seg: &[char], out: &mut String) {
    let n = seg.len();
    let mut i = 0;

    while i < n {
        let c = seg[i];
        if c == '*' || c == '_' {
            let start = i;
            let mut j = i;
            while j < n && seg[j] == c {
                j += 1;
            }

            let prev = if start > 0 { Some(seg[start - 1]) } else { None };
            let next = seg.get(j).copied();

            // A backslash-escaped delimiter is not an emphasis marker.
            let escaped = prev == Some('\\');

            if !escaped {
                let prev_cjk = prev.is_some_and(is_cjk);
                let next_cjk = next.is_some_and(is_cjk);
                let prev_punct = prev.is_some_and(is_md_punct);
                let next_punct = next.is_some_and(is_md_punct);

                if prev_punct && next_cjk {
                    // e.g. `)**라` — break the punctuation/run adjacency.
                    out.push(ZWSP);
                    out.extend(&seg[start..j]);
                } else if prev_cjk && next_punct {
                    // e.g. `어**(` — break the run/punctuation adjacency.
                    out.extend(&seg[start..j]);
                    out.push(ZWSP);
                } else {
                    out.extend(&seg[start..j]);
                }
            } else {
                out.extend(&seg[start..j]);
            }

            i = j;
        } else {
            out.push(c);
            i += 1;
        }
    }
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
    fn renders_bold_when_closing_delim_is_punct_then_cjk() {
        // Closing `**` preceded by `)` and immediately followed by a CJK letter.
        // Plain CommonMark leaves the `**` literal; the CJK fix must recover it.
        for input in [
            "**페이지 테이블(Page Table)**부터 등장한다.",
            "이것은 **가상 메모리(Virtual Memory)**라는 아이디어다.",
        ] {
            let html = render(input);
            assert!(html.contains("<strong>"), "no <strong> for {input:?}: {html}");
            assert!(html.contains("</strong>"), "no </strong> for {input:?}: {html}");
            assert!(!html.contains("**"), "literal ** leaked for {input:?}: {html}");
        }
    }

    #[test]
    fn cjk_fix_does_not_touch_inline_code() {
        let html = render("`a)**b`를 보라");
        assert!(html.contains("<code>"), "got: {html}");
        assert!(html.contains("**"), "inline code asterisks were altered: {html}");
        assert!(!html.contains("<strong>"), "got: {html}");
    }

    #[test]
    fn cjk_fix_preserves_plain_ascii_bold() {
        let html = render("Hello **world** and `code`");
        assert!(html.contains("<strong>world</strong>"), "got: {html}");
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
