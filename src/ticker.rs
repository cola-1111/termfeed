use crate::feed::FeedItem;
use crossterm::{
    cursor, execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io::{self, Write};

const TAG_COLORS: &[Color] = &[
    Color::Cyan,
    Color::Yellow,
    Color::Green,
    Color::Magenta,
    Color::Red,
    Color::Blue,
];

#[derive(Debug, Clone)]
struct StyledSegment {
    chars: Vec<char>,
    color: Option<Color>,
}

pub struct Ticker {
    segments: Vec<StyledSegment>,
    total_len: usize,
    offset: usize,
    width: u16,
}

impl Ticker {
    pub fn new(items: &[FeedItem], separator: &str, feed_names: &[String]) -> Self {
        let width = terminal::size().map(|(w, _)| w).unwrap_or(80);
        let (segments, total_len) = build_segments(items, separator, feed_names);

        Ticker {
            segments,
            total_len,
            offset: 0,
            width,
        }
    }

    pub fn update_items(&mut self, items: &[FeedItem], separator: &str, feed_names: &[String]) {
        let (segments, total_len) = build_segments(items, separator, feed_names);
        self.segments = segments;
        self.total_len = total_len;
        self.offset = 0;
    }

    pub fn update_width(&mut self, width: u16) {
        self.width = width;
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

        execute!(
            stdout,
            cursor::MoveTo(0, y),
            terminal::Clear(ClearType::CurrentLine),
        )?;

        if self.total_len == 0 {
            execute!(stdout, Print("Fetching feeds..."))?;
            stdout.flush()?;
            return Ok(());
        }

        let window = self.width as usize;
        let mut global_pos = 0;

        for seg in &self.segments {
            for (i, &ch) in seg.chars.iter().enumerate() {
                let pos_in_scroll = (global_pos + i) % self.total_len;
                let visible_pos = (pos_in_scroll + self.total_len - self.offset) % self.total_len;

                if visible_pos < window {
                    if let Some(color) = seg.color {
                        execute!(
                            stdout,
                            cursor::MoveTo(visible_pos as u16, y),
                            SetForegroundColor(color),
                            Print(ch),
                            ResetColor,
                        )?;
                    } else {
                        execute!(
                            stdout,
                            cursor::MoveTo(visible_pos as u16, y),
                            Print(ch),
                        )?;
                    }
                }
            }

            global_pos += seg.chars.len();
        }

        stdout.flush()?;
        Ok(())
    }

    pub fn advance(&mut self) {
        if self.total_len > 0 {
            self.offset = (self.offset + 1) % self.total_len;
        }
    }
}

fn build_segments(
    items: &[FeedItem],
    separator: &str,
    feed_names: &[String],
) -> (Vec<StyledSegment>, usize) {
    let mut segments = Vec::new();
    let mut total_len = 0;

    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            let chars: Vec<char> = separator.chars().collect();
            total_len += chars.len();
            segments.push(StyledSegment { chars, color: None });
        }

        let color_idx = feed_names
            .iter()
            .position(|n| n == &item.feed_name)
            .unwrap_or(0)
            % TAG_COLORS.len();

        let tag = format!("[{}] ", item.feed_name);
        let tag_chars: Vec<char> = tag.chars().collect();
        total_len += tag_chars.len();
        segments.push(StyledSegment {
            chars: tag_chars,
            color: Some(TAG_COLORS[color_idx]),
        });

        let title_chars: Vec<char> = item.title.chars().collect();
        total_len += title_chars.len();
        segments.push(StyledSegment {
            chars: title_chars,
            color: None,
        });
    }

    (segments, total_len)
}
