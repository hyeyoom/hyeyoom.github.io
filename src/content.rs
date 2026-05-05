use anyhow::{anyhow, Result};
use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Frontmatter {
    pub title: String,
    pub date: String,
    pub description: Option<String>,
    #[serde(default)]
    pub math: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostKind {
    Article,
    Page,
}

#[derive(Debug, Clone)]
pub struct Post {
    pub slug: String,
    pub frontmatter: Frontmatter,
    pub body_md: String,
    pub kind: PostKind,
}

pub fn split_frontmatter(input: &str) -> Result<(Frontmatter, String)> {
    let trimmed = input.trim_start();
    let after_open = trimmed
        .strip_prefix("+++\n")
        .ok_or_else(|| anyhow!("missing opening +++ delimiter"))?;
    let end_idx = after_open
        .find("\n+++")
        .ok_or_else(|| anyhow!("missing closing +++ delimiter"))?;
    let fm_text = &after_open[..end_idx];
    let body_start = end_idx + "\n+++".len();
    let body = after_open[body_start..].trim_start_matches('\n');
    let fm: Frontmatter = toml::from_str(fm_text)?;
    Ok((fm, body.to_string()))
}

pub fn kst() -> FixedOffset {
    FixedOffset::east_opt(9 * 3600).unwrap()
}

/// RFC 3339 datetime 또는 YYYY-MM-DD 형식을 받아 FixedOffset 시각으로 파싱.
/// date-only인 경우 KST 자정으로 해석.
pub fn parse_published(s: &str) -> Result<DateTime<FixedOffset>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt);
    }
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let naive = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| anyhow!("invalid time"))?;
        return kst()
            .from_local_datetime(&naive)
            .single()
            .ok_or_else(|| anyhow!("ambiguous local time"));
    }
    Err(anyhow!("unrecognized date format: {}", s))
}

/// 사람 친화 표시용 (KST, 분 단위).
pub fn display_kst(dt: &DateTime<FixedOffset>) -> String {
    dt.with_timezone(&kst())
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

pub fn slug_from_filename(stem: &str) -> String {
    let parts: Vec<&str> = stem.splitn(4, '-').collect();
    let dated = parts.len() == 4
        && parts[0].len() == 4
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].len() == 2
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].len() == 2
        && parts[2].chars().all(|c| c.is_ascii_digit());
    if dated {
        parts[3].to_string()
    } else {
        stem.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let input = "+++\ntitle = \"Hello\"\ndate = \"2026-05-05\"\n+++\n\nBody text\n";
        let (fm, body) = split_frontmatter(input).unwrap();
        assert_eq!(fm.title, "Hello");
        assert_eq!(fm.date, "2026-05-05");
        assert_eq!(body.trim(), "Body text");
    }

    #[test]
    fn rejects_missing_open_delimiter() {
        assert!(split_frontmatter("no frontmatter here").is_err());
    }

    #[test]
    fn rejects_missing_close_delimiter() {
        assert!(split_frontmatter("+++\ntitle = \"x\"\ndate = \"d\"\n").is_err());
    }

    #[test]
    fn defaults_math_to_false() {
        let input = "+++\ntitle = \"T\"\ndate = \"2026-05-05\"\n+++\nbody";
        let (fm, _) = split_frontmatter(input).unwrap();
        assert!(!fm.math);
    }

    #[test]
    fn extracts_slug_from_dated_stem() {
        assert_eq!(slug_from_filename("2026-05-05-hello-world"), "hello-world");
    }

    #[test]
    fn keeps_plain_stem() {
        assert_eq!(slug_from_filename("about"), "about");
    }

    #[test]
    fn date_with_wrong_pad_falls_through() {
        assert_eq!(slug_from_filename("2026-5-5-x"), "2026-5-5-x");
    }

    #[test]
    fn non_numeric_prefix_falls_through() {
        assert_eq!(slug_from_filename("xxxx-yy-zz-foo"), "xxxx-yy-zz-foo");
    }

    #[test]
    fn parses_rfc3339_kst() {
        let dt = parse_published("2026-05-05T14:30:45+09:00").unwrap();
        assert_eq!(display_kst(&dt), "2026-05-05 14:30");
    }

    #[test]
    fn parses_date_only_as_kst_midnight() {
        let dt = parse_published("2026-05-05").unwrap();
        assert_eq!(display_kst(&dt), "2026-05-05 00:00");
    }

    #[test]
    fn converts_utc_to_kst_for_display() {
        let dt = parse_published("2026-05-05T00:00:00Z").unwrap();
        assert_eq!(display_kst(&dt), "2026-05-05 09:00");
    }

    #[test]
    fn rejects_invalid_date_string() {
        assert!(parse_published("not a date").is_err());
    }
}
