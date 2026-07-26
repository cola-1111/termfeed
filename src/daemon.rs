use crate::config::Config;
use crate::feed::{self, FeedItem};
use crate::ticker::Ticker;
use crate::{DisplayMode, TerminalGuard};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};

fn cache_dir() -> anyhow::Result<PathBuf> {
    let home = crate::config::home_dir()?;
    Ok(home.join(".cache/termfeed"))
}

fn pid_path() -> anyhow::Result<PathBuf> {
    Ok(cache_dir()?.join("daemon.pid"))
}

fn cache_path() -> anyhow::Result<PathBuf> {
    Ok(cache_dir()?.join("feeds.json"))
}

fn write_cache(items: &[FeedItem]) -> anyhow::Result<()> {
    let dir = cache_dir()?;
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string(items)?;
    let tmp = dir.join("feeds.json.tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, cache_path()?)?;
    Ok(())
}

pub fn read_cache() -> anyhow::Result<Vec<FeedItem>> {
    let path = cache_path()?;
    if !path.exists() {
        anyhow::bail!(
            "No feed cache found. Start the daemon first with: termfeed --daemon"
        );
    }
    let content = std::fs::read_to_string(&path)?;
    let items: Vec<FeedItem> = serde_json::from_str(&content)?;
    Ok(items)
}

struct PidGuard;

impl PidGuard {
    fn new() -> anyhow::Result<Self> {
        let dir = cache_dir()?;
        std::fs::create_dir_all(&dir)?;
        std::fs::write(pid_path()?, std::process::id().to_string())?;
        Ok(Self)
    }
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(pid_path().unwrap());
    }
}

pub async fn run_daemon(cfg: Config) -> anyhow::Result<()> {
    let _pid_guard = PidGuard::new()?;

    let client = feed::build_client()?;
    let max_items = cfg.max_items_per_feed;
    let refresh_interval = Duration::from_secs(cfg.refresh_interval_sec);
    let feed_names: Vec<String> = cfg.feeds.iter().map(|f| f.name.clone()).collect();

    let items = feed::fetch_all_feeds(&client, &cfg.feeds, max_items).await;
    if let Err(e) = write_cache(&items) {
        eprintln!("cache write error: {}", e);
    }

    let ticker = Arc::new(RwLock::new(Ticker::new(
        &items, &cfg.separator, &feed_names,
    )));

    let _guard = TerminalGuard::setup(DisplayMode::Daemon)?;

    let ticker_clone = Arc::clone(&ticker);
    let feeds = cfg.feeds.clone();
    let separator = cfg.separator.clone();
    let feed_names_clone = feed_names.clone();
    let refresh_handle = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(refresh_interval.as_secs()));
        interval.tick().await;
        loop {
            interval.tick().await;
            let new_items = feed::fetch_all_feeds(&client, &feeds, max_items).await;
            if !new_items.is_empty() {
                if let Err(e) = write_cache(&new_items) {
                    eprintln!("cache write error: {}", e);
                }
                let mut t = ticker_clone.write().await;
                t.update_items(&new_items, &separator, &feed_names_clone);
            }
        }
    });

    let scroll_speed = Duration::from_millis(cfg.scroll_speed_ms);
    let mut scroll_interval = time::interval(scroll_speed);

    loop {
        scroll_interval.tick().await;

        {
            let t = ticker.read().await;
            t.render_pane_frame()?;
        }

        {
            let mut t = ticker.write().await;
            t.advance();
        }

        if event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.code == KeyCode::Char('q')
                        || key.code == KeyCode::Esc
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        break;
                    }
                }
                Event::Resize(w, _) => {
                    let mut t = ticker.write().await;
                    t.update_width(w);
                }
                _ => {}
            }
        }
    }

    refresh_handle.abort();
    Ok(())
}

pub fn stop_daemon() -> anyhow::Result<()> {
    let path = match pid_path() {
        Ok(p) if p.exists() => p,
        _ => {
            eprintln!("No termfeed daemon is running.");
            return Ok(());
        }
    };

    let pid_str = std::fs::read_to_string(&path)?;
    let pid: u32 = pid_str.trim().parse()?;

    #[cfg(unix)]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
        eprintln!("termfeed daemon (pid {}) stopped.", pid);
    }

    #[cfg(not(unix))]
    {
        eprintln!("Daemon stop is only supported on Unix. Remove {} manually.", path.display());
    }

    let _ = std::fs::remove_file(&path);
    Ok(())
}
