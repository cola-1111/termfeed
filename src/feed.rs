use crate::config::FeedEntry;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeedItem {
    pub feed_name: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub link: Option<String>,
}

pub fn build_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("termfeed/0.1.0")
        .build()
        .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {}", e))
}

pub async fn fetch_all_feeds(
    client: &reqwest::Client,
    feeds: &[FeedEntry],
    max_items: usize,
) -> Vec<FeedItem> {
    let mut items = Vec::new();

    for feed_entry in feeds {
        match fetch_feed(client, feed_entry, max_items).await {
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
            let description = e
                .content
                .and_then(|c| c.body)
                .or_else(|| e.summary.map(|s| s.content))
                .map(|s| strip_html(&s));

            let link = e.links.first().map(|l| l.href.clone());

            Some(FeedItem {
                feed_name: entry.name.clone(),
                title: sanitize(title.trim()),
                description,
                link,
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

fn strip_html(s: &str) -> String {
    let text = html2text::from_read(s.as_bytes(), 1000)
        .unwrap_or_else(|_| s.to_string());
    let cleaned = strip_link_refs(&text);
    sanitize(cleaned.trim())
}

fn strip_link_refs(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        result.push_str(&rest[..open]);
        let is_nested = open > 0 && rest.as_bytes()[open - 1] == b'[';
        let after_open = &rest[open + 1..];
        if let Some(close) = after_open.find(']') {
            let content = &after_open[..close];
            if !is_nested && !content.is_empty() && content.bytes().all(|b| b.is_ascii_digit()) {
                rest = &after_open[close + 1..];
            } else {
                result.push('[');
                result.push_str(content);
                result.push(']');
                rest = &after_open[close + 1..];
            }
        } else {
            result.push_str(&rest[open..]);
            rest = "";
            break;
        }
    }
    result.push_str(rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_normal_text() {
        assert_eq!(sanitize("hello world"), "hello world");
    }

    #[test]
    fn test_sanitize_removes_nul() {
        assert_eq!(sanitize("hello\x00world"), "helloworld");
    }

    #[test]
    fn test_sanitize_removes_esc() {
        assert_eq!(sanitize("hello\x1bworld"), "helloworld");
    }

    #[test]
    fn test_sanitize_removes_bel() {
        assert_eq!(sanitize("hello\x07world"), "helloworld");
    }

    #[test]
    fn test_sanitize_empty_string() {
        assert_eq!(sanitize(""), "");
    }

    #[test]
    fn test_sanitize_japanese() {
        assert_eq!(sanitize("こんにちは世界"), "こんにちは世界");
    }

    #[test]
    fn test_sanitize_mixed_control() {
        assert_eq!(sanitize("a\x00b\x1bc"), "abc");
    }

    #[test]
    fn test_strip_link_refs_removes_numeric() {
        assert_eq!(strip_link_refs("hello [1] world"), "hello  world");
    }

    #[test]
    fn test_strip_link_refs_preserves_text() {
        assert_eq!(strip_link_refs("[text]"), "[text]");
    }

    #[test]
    fn test_strip_link_refs_preserves_nested() {
        assert_eq!(strip_link_refs("[[1]]"), "[[1]]");
    }

    #[test]
    fn test_strip_link_refs_multiple() {
        assert_eq!(strip_link_refs("[1][2][3]"), "");
    }

    #[test]
    fn test_strip_link_refs_no_brackets() {
        assert_eq!(strip_link_refs("no brackets"), "no brackets");
    }

    #[test]
    fn test_strip_link_refs_unclosed() {
        assert_eq!(strip_link_refs("open [without close"), "open [without close");
    }

    #[test]
    fn test_strip_link_refs_empty_brackets() {
        assert_eq!(strip_link_refs("[]"), "[]");
    }

    #[test]
    fn test_strip_link_refs_mixed() {
        assert_eq!(strip_link_refs("text [1] [label] [2]"), "text  [label] ");
    }

    #[test]
    fn test_strip_html_simple() {
        let result = strip_html("<p>hello</p>");
        assert!(result.contains("hello"));
        assert!(!result.contains("<"));
        assert!(!result.contains(">"));
    }

    #[test]
    fn test_strip_html_empty() {
        assert_eq!(strip_html(""), "");
    }

    #[test]
    fn test_strip_html_plain_text() {
        let result = strip_html("plain text");
        assert_eq!(result.trim(), "plain text");
    }

    #[test]
    fn test_strip_html_multiple_tags() {
        let result = strip_html("<b>bold</b> and <i>italic</i>");
        assert!(result.contains("bold"));
        assert!(result.contains("and"));
        assert!(result.contains("italic"));
        assert!(!result.contains("<"));
        assert!(!result.contains(">"));
    }

    #[test]
    fn test_strip_html_no_control_chars() {
        let result = strip_html("<p>hello\x1bworld</p>");
        assert!(!result.contains("\x1b"));
    }
}
