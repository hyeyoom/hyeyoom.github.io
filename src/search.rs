use crate::content::Post;
use anyhow::Result;
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;
use pulldown_cmark::{Event, Options, Parser};
use rust_stemmers::{Algorithm, Stemmer};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
struct SearchEntry {
    title: String,
    url: String,
    date: String,
    description: Option<String>,
    excerpt: String,
    keywords: Vec<String>,
    terms: Vec<String>,
    search_text: String,
}

struct Analyzer {
    ko: Tokenizer,
    en: Stemmer,
}

impl Analyzer {
    fn new() -> Result<Self> {
        let dictionary = load_dictionary("embedded://ko-dic")?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        Ok(Self {
            ko: Tokenizer::new(segmenter),
            en: Stemmer::create(Algorithm::English),
        })
    }

    fn add_weighted_terms(&self, text: &str, weight: usize, scores: &mut HashMap<String, usize>) {
        for term in self.korean_terms(text) {
            *scores.entry(term).or_insert(0) += weight;
        }
        for term in self.english_terms(text) {
            *scores.entry(term).or_insert(0) += weight;
        }
    }

    fn korean_terms(&self, text: &str) -> Vec<String> {
        if !text.chars().any(is_hangul) {
            return Vec::new();
        }

        let Ok(mut tokens) = self.ko.tokenize(text) else {
            return Vec::new();
        };
        let mut terms = Vec::new();
        for token in tokens.iter_mut() {
            let surface = token.surface.as_ref().trim().to_string();
            if surface.chars().count() < 2 || !surface.chars().any(is_hangul) {
                continue;
            }
            let details = token.details();
            let pos = details.first().copied().unwrap_or_default();
            if matches!(
                pos,
                "NNG" | "NNP" | "NNB" | "NNBC" | "NR" | "NP" | "SL" | "SH"
            ) {
                terms.push(surface);
            }
        }
        terms
    }

    fn english_terms(&self, text: &str) -> Vec<String> {
        let mut terms = Vec::new();
        for raw in ascii_words(text) {
            let lowered = raw.to_ascii_lowercase();
            if lowered.len() < 2 || is_stopword(&lowered) {
                continue;
            }
            terms.push(lowered.clone());
            let stem = self.en.stem(&lowered).to_string();
            if stem != lowered && stem.len() >= 2 && !is_stopword(&stem) {
                terms.push(stem);
            }
        }
        terms
    }
}

pub fn build_json(posts: &[&Post]) -> Result<String> {
    let analyzer = Analyzer::new()?;
    let entries: Vec<SearchEntry> = posts
        .iter()
        .map(|post| build_entry(post, &analyzer))
        .collect();
    Ok(serde_json::to_string(&entries)?)
}

fn build_entry(post: &Post, analyzer: &Analyzer) -> SearchEntry {
    let body_text = markdown_text(&post.body_md);
    let mut scores = HashMap::new();

    analyzer.add_weighted_terms(&post.frontmatter.title, 8, &mut scores);
    if let Some(description) = &post.frontmatter.description {
        analyzer.add_weighted_terms(description, 4, &mut scores);
    }
    analyzer.add_weighted_terms(&body_text, 1, &mut scores);

    let mut ranked: Vec<(String, usize)> = scores.into_iter().collect();
    ranked.sort_by(|(a_term, a_score), (b_term, b_score)| {
        b_score.cmp(a_score).then_with(|| a_term.cmp(b_term))
    });

    let keywords = ranked
        .iter()
        .take(24)
        .map(|(term, _)| term.clone())
        .collect();
    let mut seen = HashSet::new();
    let terms = ranked
        .into_iter()
        .filter_map(|(term, _)| {
            if seen.insert(term.clone()) {
                Some(term)
            } else {
                None
            }
        })
        .take(180)
        .collect();

    SearchEntry {
        title: post.frontmatter.title.clone(),
        url: post.public_url_path(),
        date: post.frontmatter.date.clone(),
        description: post.frontmatter.description.clone(),
        excerpt: excerpt(&body_text, 180),
        keywords,
        terms,
        search_text: searchable_text(
            &post.frontmatter.title,
            post.frontmatter.description.as_deref(),
            &body_text,
        ),
    }
}

fn searchable_text(title: &str, description: Option<&str>, body: &str) -> String {
    let mut parts = vec![title];
    if let Some(description) = description {
        parts.push(description);
    }
    parts.push(body);
    parts
        .join(" ")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn markdown_text(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let mut out = String::new();
    for event in Parser::new_ext(md, opts) {
        match event {
            Event::Text(text) | Event::Code(text) => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&text);
            }
            _ => {}
        }
    }
    out
}

fn excerpt(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = normalized.chars().take(max_chars).collect();
    if normalized.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn ascii_words(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = None;
    for (idx, ch) in text.char_indices() {
        if ch.is_ascii_alphanumeric() {
            start.get_or_insert(idx);
        } else if let Some(s) = start.take() {
            words.push(&text[s..idx]);
        }
    }
    if let Some(s) = start {
        words.push(&text[s..]);
    }
    words
}

fn is_hangul(ch: char) -> bool {
    ('가'..='힣').contains(&ch)
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "com"
            | "for"
            | "from"
            | "http"
            | "https"
            | "io"
            | "in"
            | "is"
            | "it"
            | "net"
            | "of"
            | "on"
            | "or"
            | "org"
            | "that"
            | "the"
            | "this"
            | "to"
            | "www"
            | "with"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Frontmatter, PostKind};

    fn post() -> Post {
        Post {
            slug: "latency-test".into(),
            frontmatter: Frontmatter {
                title: "꼬리 지연과 Tail Latency".into(),
                date: "2026-05-17".into(),
                description: Some("큐 대기와 시스템 지연을 설명합니다".into()),
                math: false,
            },
            body_md: "지연이 커지면 큐가 쌓이고 requests are delayed.".into(),
            kind: PostKind::Article,
        }
    }

    #[test]
    fn builds_search_json_with_korean_and_english_terms() {
        let sample = post();
        let json = build_json(&[&sample]).unwrap();

        assert!(json.contains("꼬리"));
        assert!(json.contains("지연"));
        assert!(json.contains("latency"));
        assert!(json.contains("latenc"));
        assert!(json.contains("/posts/latency-test/"));
    }

    #[test]
    fn builds_translation_search_url() {
        let mut sample = post();
        sample.slug = "paper".into();
        sample.kind = PostKind::Translation;

        let json = build_json(&[&sample]).unwrap();

        assert!(json.contains("/translations/paper/"));
        assert!(!json.contains("/posts/paper/"));
    }

    #[test]
    fn extracts_plain_text_from_markdown() {
        let text = markdown_text("Hello **world**\n\n`latency` [link](https://example.com)");

        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(text.contains("latency"));
        assert!(!text.contains("https://example.com"));
    }

    #[test]
    fn does_not_promote_url_protocol_parts() {
        let mut sample = post();
        sample.body_md = "참고: <https://example.com/docs/search>".into();

        let json = build_json(&[&sample]).unwrap();

        assert!(!json.contains("\"https\""));
        assert!(!json.contains("\"com\""));
    }

    #[test]
    fn keeps_deep_english_body_terms_searchable() {
        let mut sample = post();
        sample.body_md = format!("{} Invariants define correctness.", "noise ".repeat(240));

        let json = build_json(&[&sample]).unwrap();

        assert!(json.contains("\"search_text\""));
        assert!(json.contains("invariants"));
    }
}
