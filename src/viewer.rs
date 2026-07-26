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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(name: &str, title: &str) -> FeedItem {
        FeedItem {
            feed_name: name.to_string(),
            title: title.to_string(),
            description: None,
            link: None,
        }
    }

    #[test]
    fn test_unique_feed_names_empty() {
        let items: Vec<FeedItem> = vec![];
        assert_eq!(unique_feed_names(&items), Vec::<String>::new());
    }

    #[test]
    fn test_unique_feed_names_dedup() {
        let items = vec![
            make_item("news", "title1"),
            make_item("tech", "title2"),
            make_item("news", "title3"),
        ];
        let result = unique_feed_names(&items);
        assert_eq!(result, vec!["news", "tech"]);
    }

    #[test]
    fn test_unique_feed_names_all_same() {
        let items = vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
        ];
        assert_eq!(unique_feed_names(&items), vec!["news"]);
    }

    #[test]
    fn test_viewerapp_new_empty() {
        let app = ViewerApp::new(vec![]);
        assert_eq!(app.list_state.selected(), None);
    }

    #[test]
    fn test_viewerapp_new_nonempty() {
        let items = vec![make_item("news", "title1")];
        let app = ViewerApp::new(items);
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn test_handle_key_q() {
        let mut app = ViewerApp::new(vec![make_item("news", "title1")]);
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(app.handle_key(key));
    }

    #[test]
    fn test_handle_key_esc() {
        let mut app = ViewerApp::new(vec![make_item("news", "title1")]);
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.handle_key(key));
    }

    #[test]
    fn test_handle_key_ctrl_c() {
        let mut app = ViewerApp::new(vec![make_item("news", "title1")]);
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(app.handle_key(key));
    }

    #[test]
    fn test_handle_key_j() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
        ]);
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert!(!app.handle_key(key));
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn test_handle_key_k() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
        ]);
        app.list_state.select(Some(1));
        let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert!(!app.handle_key(key));
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn test_handle_key_other() {
        let mut app = ViewerApp::new(vec![make_item("news", "title1")]);
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(!app.handle_key(key));
    }

    #[test]
    fn test_move_up_from_middle() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
            make_item("news", "title3"),
        ]);
        app.list_state.select(Some(2));
        app.move_up();
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn test_move_up_from_start() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
        ]);
        app.list_state.select(Some(0));
        app.move_up();
        assert_eq!(app.list_state.selected(), Some(0));
    }

    #[test]
    fn test_move_down_from_start() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
        ]);
        app.list_state.select(Some(0));
        app.move_down();
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn test_move_down_from_end() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
            make_item("news", "title3"),
        ]);
        app.list_state.select(Some(2));
        app.move_down();
        assert_eq!(app.list_state.selected(), Some(2));
    }

    #[test]
    fn test_move_up_empty_items() {
        let mut app = ViewerApp::new(vec![]);
        app.move_up();
        assert_eq!(app.list_state.selected(), None);
    }

    #[test]
    fn test_move_down_empty_items() {
        let mut app = ViewerApp::new(vec![]);
        app.move_down();
        assert_eq!(app.list_state.selected(), None);
    }

    #[test]
    fn test_update_items_clamp_index() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
            make_item("news", "title3"),
        ]);
        app.list_state.select(Some(2));
        app.update_items(vec![
            make_item("news", "title1"),
            make_item("tech", "title2"),
        ]);
        assert_eq!(app.list_state.selected(), Some(1));
    }

    #[test]
    fn test_update_items_empty() {
        let mut app = ViewerApp::new(vec![make_item("news", "title1")]);
        app.update_items(vec![]);
        assert_eq!(app.list_state.selected(), None);
    }

    #[test]
    fn test_update_items_preserve() {
        let mut app = ViewerApp::new(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
        ]);
        app.list_state.select(Some(1));
        app.update_items(vec![
            make_item("news", "title1"),
            make_item("news", "title2"),
            make_item("news", "title3"),
        ]);
        assert_eq!(app.list_state.selected(), Some(1));
    }
}
