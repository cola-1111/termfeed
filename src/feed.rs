use crate::config::FeedEntry;

#[derive(Debug, Clone)]
pub struct FeedItem {
    pub feed_name: String,
    pub title: String,
}

pub async fn fetch_all_feeds(
    feeds: &[FeedEntry],
    max_items: usize,
) -> Vec<FeedItem> {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("termfeed/0.1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to build HTTP client: {}", e);
            return Vec::new();
        }
    };

    let mut items = Vec::new();

    for feed_entry in feeds {
        match fetch_feed(&client, feed_entry, max_items).await {
            Ok(feed_items) => items.extend(feed_items),
            Err(e) => eprintln!("[{}] fetch error: {}", feed_entry.name, e),
        }
    }

    items
}

async fn fetch_feed(
    client: &reqwest::Client,
    entry: &FeedEntry,
    max_items: usize,
) -> anyhow::Result<Vec<FeedItem>> {
    let response = client.get(&entry.url).send().await?;
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {}", status);
    }
    let body = response.bytes().await?;
    let feed = feed_rs::parser::parse(&body[..])
        .map_err(|e| anyhow::anyhow!("parse error (got {} bytes): {}", body.len(), e))?;

    let items: Vec<FeedItem> = feed
        .entries
        .into_iter()
        .take(max_items)
        .filter_map(|e| {
            let title = e.title.map(|t| t.content)?;
            if title.trim().is_empty() {
                return None;
            }
            Some(FeedItem {
                feed_name: entry.name.clone(),
                title: sanitize(title.trim()),
            })
        })
        .collect();

    Ok(items)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && *c != '\x1b' && *c != '\x07')
        .collect()
}
