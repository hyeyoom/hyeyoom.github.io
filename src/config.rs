use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub title: String,
    pub author: String,
    pub description: String,
    pub base_url: String,
    pub language: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&raw)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn loads_config_from_toml() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            f,
            "title = \"Site\"\nauthor = \"Me\"\ndescription = \"d\"\nbase_url = \"https://example.com\"\nlanguage = \"ko\""
        )
        .unwrap();
        let cfg = Config::load(f.path()).unwrap();
        assert_eq!(cfg.title, "Site");
        assert_eq!(cfg.base_url, "https://example.com");
        assert_eq!(cfg.language, "ko");
    }
}
