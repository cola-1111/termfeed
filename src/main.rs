mod config;
mod feed;
mod ticker;
mod titlebar;

use clap::Parser;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};

#[derive(Parser)]
#[command(name = "termfeed", about = "Terminal RSS ticker – LED marquee style")]
struct Cli {
    #[arg(short, long, help = "Path to config.toml")]
    config: Option<PathBuf>,

    #[arg(long, help = "Fetch once and exit (no scroll)")]
    once: bool,

    #[arg(long, help = "Run in pane mode (no alternate screen, for herdr/tmux pane)")]
    pane: bool,

    #[arg(long, help = "Run as background daemon (updates terminal title bar)")]
    daemon: bool,

    #[arg(long, help = "Stop the background daemon")]
    stop: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum DisplayMode {
    Fullscreen,
    Pane,
}

struct TerminalGuard {
    mode: DisplayMode,
}

impl TerminalGuard {
    fn setup(mode: DisplayMode) -> anyhow::Result<Self> {
        terminal::enable_raw_mode()?;
        match mode {
            DisplayMode::Fullscreen => {
                execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
            }
            DisplayMode::Pane => {
                execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
            }
        }
        Ok(Self { mode })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        match self.mode {
            DisplayMode::Fullscreen => {
                let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
            }
            DisplayMode::Pane => {
                let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
            }
        }
        let _ = terminal::disable_raw_mode();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.stop {
        titlebar::stop_daemon()?;
        return Ok(());
    }

    let cfg = config::load_config(cli.config)?;

    if cli.once {
        let client = feed::build_client()?;
        let items = feed::fetch_all_feeds(&client, &cfg.feeds, cfg.max_items_per_feed).await;
        for item in &items {
            println!("[{}] {}", item.feed_name, item.title);
        }
        return Ok(());
    }

    if cli.daemon {
        return titlebar::run_daemon(cfg).await;
    }

    let mode = if cli.pane {
        DisplayMode::Pane
    } else {
        DisplayMode::Fullscreen
    };

    run_ticker(cfg, mode).await
}

async fn run_ticker(cfg: config::Config, mode: DisplayMode) -> anyhow::Result<()> {
    let feed_names: Vec<String> = cfg.feeds.iter().map(|f| f.name.clone()).collect();
    let client = feed::build_client()?;

    let items = feed::fetch_all_feeds(&client, &cfg.feeds, cfg.max_items_per_feed).await;
    let ticker = Arc::new(RwLock::new(ticker::Ticker::new(
        &items,
        &cfg.separator,
        &feed_names,
    )));

    let _guard = TerminalGuard::setup(mode)?;

    let ticker_clone = Arc::clone(&ticker);
    let feeds = cfg.feeds.clone();
    let separator = cfg.separator.clone();
    let feed_names_clone = feed_names.clone();
    let max_items = cfg.max_items_per_feed;
    let refresh_sec = cfg.refresh_interval_sec;

    let refresh_handle = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(refresh_sec));
        interval.tick().await;
        loop {
            interval.tick().await;
            let new_items = feed::fetch_all_feeds(&client, &feeds, max_items).await;
            if !new_items.is_empty() {
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
            match mode {
                DisplayMode::Fullscreen => t.render_frame()?,
                DisplayMode::Pane => t.render_pane_frame()?,
            }
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
    // _guard dropped here → terminal restored even on panic

    Ok(())
}
