use crate::config::Config;
use crate::feed;
use std::io::{self, Write};
use std::path::PathBuf;
use tokio::time::{self, Duration};

fn pid_file_path() -> PathBuf {
    let mut p = crate::config::home_dir().unwrap_or_else(|_| std::env::temp_dir());
    p.push(".termfeed-daemon.pid");
    p
}

struct PidGuard;

impl PidGuard {
    fn new() -> anyhow::Result<Self> {
        let pid = std::process::id();
        std::fs::write(pid_file_path(), pid.to_string())?;
        Ok(Self)
    }
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(pid_file_path());
        set_terminal_title("");
    }
}

pub async fn run_daemon(cfg: Config) -> anyhow::Result<()> {
    let _pid_guard = PidGuard::new()?;

    eprintln!("termfeed daemon started (pid {}). Headlines will appear in your terminal title bar.", std::process::id());
    eprintln!("Run `termfeed --stop` to stop, or `termfeed` to open the full view.");

    let max_items = cfg.max_items_per_feed;
    let title_scroll_interval = Duration::from_secs(3);
    let refresh_interval = Duration::from_secs(cfg.refresh_interval_sec);
    let client = feed::build_client()?;

    let mut items = feed::fetch_all_feeds(&client, &cfg.feeds, max_items).await;
    let mut current_idx: usize = 0;
    let mut scroll_ticker = time::interval(title_scroll_interval);
    let mut refresh_ticker = time::interval(refresh_interval);
    refresh_ticker.tick().await;

    let shutdown = async {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");
            tokio::select! {
                _ = ctrl_c => {}
                _ = sigterm.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            ctrl_c.await.ok();
        }
    };
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = scroll_ticker.tick() => {
                if !items.is_empty() {
                    let item = &items[current_idx % items.len()];
                    set_terminal_title(&format!(
                        "[{}] {}",
                        item.feed_name, item.title
                    ));
                    current_idx = (current_idx + 1) % items.len();
                } else {
                    set_terminal_title("termfeed: no feeds loaded");
                }
            }
            _ = refresh_ticker.tick() => {
                let new_items = feed::fetch_all_feeds(&client, &cfg.feeds, max_items).await;
                if !new_items.is_empty() {
                    items = new_items;
                    current_idx = 0;
                }
            }
            _ = &mut shutdown => {
                break;
            }
        }
    }

    Ok(())
}

pub fn stop_daemon() -> anyhow::Result<()> {
    let pid_path = pid_file_path();
    if !pid_path.exists() {
        eprintln!("No termfeed daemon is running.");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: u32 = pid_str.trim().parse()?;

    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if ret == 0 {
            eprintln!("termfeed daemon (pid {}) stopped.", pid);
        } else {
            eprintln!("Could not stop daemon (pid {}). It may have already exited.", pid);
        }
    }

    let _ = std::fs::remove_file(&pid_path);
    set_terminal_title("");
    Ok(())
}

fn set_terminal_title(title: &str) {
    let sanitized: String = title
        .chars()
        .filter(|c| !c.is_control() && *c != '\x1b' && *c != '\x07')
        .collect();
    let mut stdout = io::stdout();
    let _ = write!(stdout, "\x1b]0;{}\x07", sanitized);
    let _ = stdout.flush();
}

