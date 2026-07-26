use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_scroll_speed")]
    pub scroll_speed_ms: u64,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_sec: u64,
    #[serde(default = "default_separator")]
    pub separator: String,
    #[serde(default = "default_max_items")]
    pub max_items_per_feed: usize,
    pub feeds: Vec<FeedEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeedEntry {
    pub name: String,
    pub url: String,
}

fn default_scroll_speed() -> u64 {
    100
}
fn default_refresh_interval() -> u64 {
    300
}
fn default_separator() -> String {
    " | ".to_string()
}
fn default_max_items() -> usize {
    5
}

pub fn load_config(path: Option<PathBuf>) -> anyhow::Result<Config> {
    let config_path = match path {
        Some(p) => p,
        None => {
            let home = home_dir()?;
            let candidates = [
                home.join(".config/termfeed/config.toml"),
                home.join(".termfeed.toml"),
            ];
            candidates
                .into_iter()
                .find(|p| p.exists())
                .ok_or_else(|| anyhow::anyhow!(
                    "No config file found. Create ~/.config/termfeed/config.toml or pass --config"
                ))?
        }
    };

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", config_path.display(), e))?;
    let config: Config = toml::from_str(&content)?;

    if config.feeds.is_empty() {
        anyhow::bail!("No feeds configured. Add [[feeds]] entries to your config file.");
    }

    if config.scroll_speed_ms == 0 {
        anyhow::bail!("scroll_speed_ms must be > 0");
    }

    if config.refresh_interval_sec == 0 {
        anyhow::bail!("refresh_interval_sec must be > 0");
    }

    Ok(config)
}

pub fn home_dir() -> anyhow::Result<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| anyhow::anyhow!("HOME environment variable not set"))
    }
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .map_err(|_| anyhow::anyhow!("USERPROFILE environment variable not set"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_scroll_speed() {
        assert_eq!(default_scroll_speed(), 100);
    }

    #[test]
    fn test_default_refresh_interval() {
        assert_eq!(default_refresh_interval(), 300);
    }

    #[test]
    fn test_default_separator() {
        assert_eq!(default_separator(), " | ");
    }

    #[test]
    fn test_default_max_items() {
        assert_eq!(default_max_items(), 5);
    }
}
