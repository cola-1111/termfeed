use crate::feed::FeedItem;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

const TAG_COLORS: &[Color] = &[
    Color::Cyan,
    Color::Yellow,
    Color::Green,
    Color::Magenta,
    Color::Red,
    Color::Blue,
];

pub struct ViewerApp {
    items: Vec<FeedItem>,
    feed_names: Vec<String>,
    list_state: ListState,
}

impl ViewerApp {
    pub fn new(items: Vec<FeedItem>) -> Self {
        let feed_names = unique_feed_names(&items);
        let mut list_state = ListState::default();
        if !items.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            items,
            feed_names,
            list_state,
        }
    }

    pub fn update_items(&mut self, items: Vec<FeedItem>) {
        self.feed_names = unique_feed_names(&items);
        let prev_selected = self.list_state.selected().unwrap_or(0);
        self.items = items;
        if self.items.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state
                .select(Some(prev_selected.min(self.items.len() - 1)));
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return true,
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            _ => {}
        }
        false
    }

    fn move_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some(i.saturating_sub(1)));
    }

    fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = self.list_state.selected().unwrap_or(0);
        self.list_state
            .select(Some((i + 1).min(self.items.len() - 1)));
    }

    fn selected_item(&self) -> Option<&FeedItem> {
        self.list_state.selected().and_then(|i| self.items.get(i))
    }

    fn tag_color(&self, feed_name: &str) -> Color {
        let idx = self
            .feed_names
            .iter()
            .position(|n| n == feed_name)
            .unwrap_or(0)
            % TAG_COLORS.len();
        TAG_COLORS[idx]
    }
}

pub fn render(app: &mut ViewerApp, frame: &mut Frame) {
    let chunks = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(frame.area());

    let list_items: Vec<ListItem> = app
        .items
        .iter()
        .map(|item| {
            let color = app.tag_color(&item.feed_name);
            let line = Line::from(vec![
                Span::styled(
                    format!("[{}] ", item.feed_name),
                    Style::default().fg(color),
                ),
                Span::raw(&item.title),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(" Feeds "))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, chunks[0], &mut app.list_state);

    let detail = if let Some(item) = app.selected_item() {
        let color = app.tag_color(&item.feed_name);
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&item.feed_name, Style::default().fg(color)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                &item.title,
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if let Some(desc) = &item.description {
            if !desc.is_empty() {
                lines.push(Line::from(desc.as_str()));
                lines.push(Line::from(""));
            }
        }

        if let Some(url) = &item.link {
            lines.push(Line::from(vec![
                Span::styled("URL: ", Style::default().fg(Color::DarkGray)),
                Span::styled(url.as_str(), Style::default().fg(Color::Cyan)),
            ]));
        }

        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Detail "))
            .wrap(Wrap { trim: true })
    } else {
        Paragraph::new("No items")
            .block(Block::default().borders(Borders::ALL).title(" Detail "))
    };

    frame.render_widget(detail, chunks[1]);
}

fn unique_feed_names(items: &[FeedItem]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    items
        .iter()
        .map(|i| i.feed_name.clone())
        .filter(|n| seen.insert(n.clone()))
        .collect()
}
