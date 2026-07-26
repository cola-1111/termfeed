use crate::feed::FeedItem;
use crossterm::{
    cursor, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

const TAG_COLORS: &[Color] = &[
    Color::Cyan,
    Color::Yellow,
    Color::Green,
    Color::Magenta,
    Color::Red,
    Color::Blue,
];

#[derive(Debug, Clone)]
struct StyledChar {
    ch: char,
    width: usize,
    color: Option<Color>,
}

pub struct Ticker {
    chars: Vec<StyledChar>,
    total_width: usize,
    offset: usize,
    term_width: u16,
}

impl Ticker {
    pub fn new(items: &[FeedItem], separator: &str, feed_names: &[String]) -> Self {
        let term_width = terminal::size().map(|(w, _)| w).unwrap_or(80);
        let (chars, total_width) = build_chars(items, separator, feed_names);

        Ticker {
            chars,
            total_width,
            offset: 0,
            term_width,
        }
    }

    pub fn update_items(&mut self, items: &[FeedItem], separator: &str, feed_names: &[String]) {
        let (chars, total_width) = build_chars(items, separator, feed_names);
        self.chars = chars;
        self.total_width = total_width;
        if total_width > 0 {
            self.offset = self.offset % total_width;
        } else {
            self.offset = 0;
        }
    }

    pub fn update_width(&mut self, width: u16) {
        self.term_width = width;
    }

    pub fn render_frame(&self) -> io::Result<()> {
        let (_, rows) = terminal::size()?;
        let y = rows.saturating_sub(1);
        self.render_at_row(y)
    }

    pub fn render_pane_frame(&self) -> io::Result<()> {
        self.render_at_row(0)
    }

    fn render_at_row(&self, y: u16) -> io::Result<()> {
        let mut stdout = io::stdout();

        queue!(
            stdout,
            cursor::MoveTo(0, y),
            terminal::Clear(ClearType::CurrentLine),
        )?;

        if self.total_width == 0 {
            queue!(stdout, Print("Fetching feeds..."))?;
            stdout.flush()?;
            return Ok(());
        }

        let window = self.term_width as usize;

        // Collect visible chars into a line buffer
        // Each entry: (column, char, width, color)
        let mut visible: Vec<(usize, char, Option<Color>)> = Vec::new();
        let mut col_cursor = 0;

        for sc in &self.chars {
            let visual_col = (col_cursor + self.total_width - self.offset) % self.total_width;

            if visual_col < window {
                if visual_col + sc.width <= window {
                    visible.push((visual_col, sc.ch, sc.color));
                } else {
                    // Wide char doesn't fit — fill remaining columns with spaces
                    for pad in 0..(window - visual_col) {
                        visible.push((visual_col + pad, ' ', None));
                    }
                }
            }
            col_cursor += sc.width;
        }

        visible.sort_by_key(|v| v.0);

        // Write in runs of same color at sequential positions
        let mut current_color: Option<Color> = None;
        let mut run = String::new();
        let mut run_start: usize = 0;
        let mut run_end: usize = 0;

        for &(col, ch, color) in &visible {
            if color != current_color || col != run_end {
                if !run.is_empty() {
                    flush_run(&mut stdout, &run, current_color, y, run_start)?;
                    run.clear();
                }
                current_color = color;
                run_start = col;
                run_end = col;
            }
            run.push(ch);
            run_end += ch.width().unwrap_or(1);
        }
        if !run.is_empty() {
            flush_run(&mut stdout, &run, current_color, y, run_start)?;
        }

        stdout.flush()?;
        Ok(())
    }

    pub fn advance(&mut self) {
        if self.total_width > 0 {
            self.offset = (self.offset + 1) % self.total_width;
        }
    }
}

fn flush_run(
    stdout: &mut io::Stdout,
    run: &str,
    color: Option<Color>,
    y: u16,
    start_col: usize,
) -> io::Result<()> {
    queue!(stdout, cursor::MoveTo(start_col as u16, y))?;
    if let Some(c) = color {
        queue!(stdout, SetForegroundColor(c), Print(run), ResetColor)?;
    } else {
        queue!(stdout, Print(run))?;
    }
    Ok(())
}

fn build_chars(
    items: &[FeedItem],
    separator: &str,
    feed_names: &[String],
) -> (Vec<StyledChar>, usize) {
    let mut chars = Vec::new();
    let mut total_width = 0;

    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            for ch in separator.chars() {
                let w = ch.width().unwrap_or(1);
                total_width += w;
                chars.push(StyledChar { ch, width: w, color: None });
            }
        }

        let color_idx = feed_names
            .iter()
            .position(|n| n == &item.feed_name)
            .unwrap_or(0)
            % TAG_COLORS.len();
        let color = Some(TAG_COLORS[color_idx]);

        let tag = format!("[{}] ", item.feed_name);
        for ch in tag.chars() {
            let w = ch.width().unwrap_or(1);
            total_width += w;
            chars.push(StyledChar { ch, width: w, color });
        }

        for ch in item.title.chars() {
            let w = ch.width().unwrap_or(1);
            total_width += w;
            chars.push(StyledChar { ch, width: w, color: None });
        }
    }

    (chars, total_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::FeedItem;

    #[test]
    fn test_build_chars_empty() {
        let (chars, total_width) = build_chars(&[], " | ", &[]);
        assert_eq!(chars.len(), 0);
        assert_eq!(total_width, 0);
    }

    #[test]
    fn test_build_chars_single_item() {
        let items = vec![FeedItem {
            feed_name: "news".to_string(),
            title: "hello".to_string(),
            description: None,
            link: None,
        }];
        let feed_names = vec!["news".to_string()];

        let (chars, total_width) = build_chars(&items, " | ", &feed_names);

        // "[news] " (7 chars) + "hello" (5 chars) = 12 total width
        assert_eq!(total_width, 12);
        assert!(!chars.is_empty());
        // Tag chars should have color
        assert!(chars[0].color.is_some());
        // Find first title char (first char without color)
        let first_title = chars.iter().position(|c| c.color.is_none()).unwrap();
        assert_eq!(chars[first_title].ch, 'h');
    }

    #[test]
    fn test_build_chars_multiple_items_with_separator() {
        let items = vec![
            FeedItem {
                feed_name: "a".to_string(),
                title: "x".to_string(),
                description: None,
                link: None,
            },
            FeedItem {
                feed_name: "b".to_string(),
                title: "y".to_string(),
                description: None,
                link: None,
            },
        ];
        let feed_names = vec!["a".to_string(), "b".to_string()];

        let (chars, total_width) = build_chars(&items, " | ", &feed_names);

        // "[a] " (4) + "x" (1) + " | " (3) + "[b] " (4) + "y" (1) = 13
        assert_eq!(total_width, 13);
        assert!(!chars.is_empty());
    }

    #[test]
    fn test_build_chars_japanese_width() {
        let items = vec![FeedItem {
            feed_name: "news".to_string(),
            title: "テスト".to_string(), // 3 Japanese chars, each width 2
            description: None,
            link: None,
        }];
        let feed_names = vec!["news".to_string()];

        let (_chars, total_width) = build_chars(&items, " | ", &feed_names);

        // [news] (7: [ n e w s ] space) + テスト (6 width) = 13
        assert_eq!(total_width, 13);
    }

    #[test]
    fn test_build_chars_feed_color_cycling() {
        let items = vec![
            FeedItem {
                feed_name: "feed1".to_string(),
                title: "x".to_string(),
                description: None,
                link: None,
            },
            FeedItem {
                feed_name: "feed2".to_string(),
                title: "y".to_string(),
                description: None,
                link: None,
            },
        ];
        let feed_names = vec!["feed1".to_string(), "feed2".to_string()];

        let (chars, _) = build_chars(&items, " | ", &feed_names);

        let color1 = chars[0].color;
        // Find the second tag's first char (after separator following first item)
        let second_tag_start = chars
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, c)| c.color.is_some() && c.color != color1)
            .map(|(i, _)| i);
        assert!(second_tag_start.is_some());
        let color2 = chars[second_tag_start.unwrap()].color;
        assert_ne!(color1, color2);
    }

    #[test]
    fn test_ticker_advance_normal() {
        let items = vec![FeedItem {
            feed_name: "a".to_string(),
            title: "hello".to_string(),
            description: None,
            link: None,
        }];
        let feed_names = vec!["a".to_string()];
        let (chars, total_width) = build_chars(&items, " | ", &feed_names);

        let mut ticker = Ticker {
            chars,
            total_width,
            offset: 0,
            term_width: 80,
        };

        ticker.advance();
        assert_eq!(ticker.offset, 1);

        ticker.advance();
        assert_eq!(ticker.offset, 2);
    }

    #[test]
    fn test_ticker_advance_wraparound() {
        let items = vec![FeedItem {
            feed_name: "a".to_string(),
            title: "x".to_string(),
            description: None,
            link: None,
        }];
        let feed_names = vec!["a".to_string()];
        let (chars, total_width) = build_chars(&items, " | ", &feed_names);

        let mut ticker = Ticker {
            chars,
            total_width,
            offset: total_width - 1,
            term_width: 80,
        };

        ticker.advance();
        assert_eq!(ticker.offset, 0);
    }

    #[test]
    fn test_ticker_advance_zero_width() {
        let mut ticker = Ticker {
            chars: vec![],
            total_width: 0,
            offset: 0,
            term_width: 80,
        };

        ticker.advance();
        assert_eq!(ticker.offset, 0);
    }

    #[test]
    fn test_ticker_update_items_offset_preserved() {
        let items1 = vec![FeedItem {
            feed_name: "a".to_string(),
            title: "hello world".to_string(),
            description: None,
            link: None,
        }];
        let feed_names = vec!["a".to_string()];
        let (chars, total_width) = build_chars(&items1, " | ", &feed_names);

        let mut ticker = Ticker {
            chars,
            total_width,
            offset: 5,
            term_width: 80,
        };

        // Update with new items
        let items2 = vec![FeedItem {
            feed_name: "b".to_string(),
            title: "test".to_string(),
            description: None,
            link: None,
        }];
        let feed_names2 = vec!["b".to_string()];

        ticker.update_items(&items2, " | ", &feed_names2);

        // offset should be preserved, modulo new total_width
        assert!(ticker.offset < ticker.total_width);
    }

    #[test]
    fn test_ticker_update_items_empty() {
        let items = vec![FeedItem {
            feed_name: "a".to_string(),
            title: "x".to_string(),
            description: None,
            link: None,
        }];
        let feed_names = vec!["a".to_string()];
        let (chars, total_width) = build_chars(&items, " | ", &feed_names);

        let mut ticker = Ticker {
            chars,
            total_width,
            offset: 3,
            term_width: 80,
        };

        ticker.update_items(&[], " | ", &[]);
        assert_eq!(ticker.offset, 0);
        assert_eq!(ticker.total_width, 0);
    }

    #[test]
    fn test_ticker_update_width() {
        let items = vec![FeedItem {
            feed_name: "a".to_string(),
            title: "x".to_string(),
            description: None,
            link: None,
        }];
        let feed_names = vec!["a".to_string()];
        let (chars, total_width) = build_chars(&items, " | ", &feed_names);

        let mut ticker = Ticker {
            chars,
            total_width,
            offset: 0,
            term_width: 80,
        };

        ticker.update_width(120);
        assert_eq!(ticker.term_width, 120);
    }
}
