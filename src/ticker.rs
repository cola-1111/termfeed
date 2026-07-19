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
