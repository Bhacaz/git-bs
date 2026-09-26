use crate::{git::Repository, picker::Picker};
use crossterm::{
    cursor::Show,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::io;
use unicode_width::UnicodeWidthStr;

const BLUE: Color = Color::Rgb(102, 153, 204);
const LABEL: Color = Color::Rgb(153, 204, 255);
const MATCH: Color = Color::Yellow;

struct TerminalGuard;

fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        Show
    );
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

pub fn select(repo: &Repository, picker: &mut Picker) -> io::Result<bool> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        original_hook(info);
    }));
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut state = ListState::default();
    let mut preview_branch = None;
    let mut preview = String::new();
    loop {
        let selected = picker
            .filtered
            .get(picker.selected)
            .map(|matched| matched.index);
        if selected != preview_branch {
            preview = match picker.selected_branch() {
                Some(branch) => repo
                    .preview(branch)
                    .unwrap_or_else(|error| format!("Cannot load preview:\n{error}")),
                None => String::new(),
            };
            preview_branch = selected;
        }
        state.select(selected.map(|_| picker.selected));
        terminal.draw(|frame| draw(frame, picker, &mut state, &preview))?;
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let control = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Esc => return Ok(false),
                    KeyCode::Char('c') if control => return Ok(false),
                    KeyCode::Enter if picker.selected_branch().is_some() => return Ok(true),
                    KeyCode::Up => picker.move_selection(-1),
                    KeyCode::Down => picker.move_selection(1),
                    KeyCode::Char('p') if control => picker.move_selection(-1),
                    KeyCode::Char('n') if control => picker.move_selection(1),
                    KeyCode::PageUp => picker.move_selection(
                        -(terminal.size()?.height.saturating_sub(8) as isize).max(1),
                    ),
                    KeyCode::PageDown => picker.move_selection(
                        (terminal.size()?.height.saturating_sub(8) as isize).max(1),
                    ),
                    KeyCode::Home => picker.selected = 0,
                    KeyCode::End => picker.selected = picker.filtered.len().saturating_sub(1),
                    KeyCode::Backspace => {
                        picker.query.pop();
                        picker.filter();
                    }
                    KeyCode::Char('u') if control => {
                        picker.query.clear();
                        picker.filter();
                    }
                    KeyCode::Char(c)
                        if !control
                            && !key.modifiers.contains(KeyModifiers::ALT)
                            && !c.is_control() =>
                    {
                        picker.query.push(c);
                        picker.filter();
                    }
                    _ => {}
                }
            }
            Event::Paste(text) => {
                picker
                    .query
                    .extend(text.chars().filter(|c| !c.is_control()));
                picker.filter();
            }
            _ => {}
        }
    }
}

fn block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BLUE))
        .title(Span::styled(title, Style::default().fg(LABEL)))
}

fn branch_name_spans<'a>(name: &'a str, positions: &[usize], normal: Style) -> Vec<Span<'a>> {
    let matched = Style::default().fg(MATCH).add_modifier(Modifier::BOLD);
    let mut spans = Vec::new();
    let mut start = 0;
    let mut in_match = positions.binary_search(&0).is_ok();
    for (byte, _) in name.char_indices().skip(1) {
        let next_match = positions.binary_search(&byte).is_ok();
        if next_match != in_match {
            spans.push(Span::styled(
                &name[start..byte],
                if in_match { matched } else { normal },
            ));
            start = byte;
            in_match = next_match;
        }
    }
    spans.push(Span::styled(
        &name[start..],
        if in_match { matched } else { normal },
    ));
    spans
}

fn draw(frame: &mut Frame, picker: &Picker, state: &mut ListState, preview: &str) {
    let area = frame.area();
    if area.width < 24 || area.height < 9 {
        frame.render_widget(Paragraph::new("Enlarge terminal\nEsc: cancel"), area);
        return;
    }
    let outer = block(" Git Branch Select ");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new("↑↓ navigate • Enter: checkout • Esc: cancel")
            .style(Style::default().fg(Color::DarkGray)),
        rows[0],
    );

    let search = block(" Select Branch ");
    let search_area = search.inner(rows[1]);
    frame.render_widget(search, rows[1]);
    // Show the tail of long queries and place the cursor by display width, not bytes.
    let available = search_area.width.saturating_sub(3) as usize;
    let mut tail = picker.query.as_str();
    while tail.width() > available {
        tail = &tail[tail.chars().next().map_or(0, char::len_utf8)..];
    }
    frame.render_widget(Paragraph::new(format!("> {tail}")), search_area);
    frame.set_cursor_position((search_area.x + 2 + tail.width() as u16, search_area.y));

    let columns = if area.width >= 80 {
        Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(rows[2])
    } else {
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(rows[2])
    };
    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .map(|branch_match| {
            let branch = &picker.branches[branch_match.index];
            let normal = if branch.current {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            let mut spans = vec![Span::styled(
                if branch.current { "* " } else { "  " },
                Style::default().fg(Color::Green),
            )];
            spans.extend(branch_name_spans(
                &branch.name,
                &branch_match.positions,
                normal,
            ));
            spans.push(Span::styled(
                format!("  ({})", branch.age),
                Style::default().fg(Color::DarkGray),
            ));
            ListItem::new(Line::from(spans))
        })
        .collect();
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new("No matching branches").block(block(" Branches ")),
            columns[0],
        );
    } else {
        frame.render_stateful_widget(
            List::new(items)
                .block(block(" Branches "))
                .highlight_symbol("› ")
                .highlight_style(
                    Style::default()
                        .bg(Color::Rgb(32, 48, 64))
                        .add_modifier(Modifier::BOLD),
                ),
            columns[0],
            state,
        );
    }
    frame.render_widget(
        Paragraph::new(preview)
            .block(block(" Last 15 commits "))
            .style(Style::default().fg(LABEL)),
        columns[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::Branch;
    use ratatui::backend::TestBackend;

    #[test]
    fn renders_wide_narrow_and_tiny_terminals_with_long_unicode_query() {
        let picker = Picker::new(
            vec![Branch {
                name: "main".into(),
                age: "1 day ago".into(),
                current: true,
            }],
            "été界".repeat(100),
        );
        for (width, height) in [(120, 30), (50, 20), (24, 9), (8, 3), (0, 0)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    draw(
                        frame,
                        &picker,
                        &mut ListState::default(),
                        "* abc1234 Initial commit",
                    )
                })
                .unwrap();
        }
    }

    #[test]
    fn highlights_only_matching_letters_in_selected_and_unselected_branches() {
        let picker = Picker::new(
            vec![
                Branch {
                    name: "feature/login".into(),
                    age: "today".into(),
                    current: true,
                },
                Branch {
                    name: "fix/tooling".into(),
                    age: "yesterday".into(),
                    current: false,
                },
            ],
            "ftlg".into(),
        );
        let mut terminal = Terminal::new(TestBackend::new(120, 20)).unwrap();
        let mut state = ListState::default();
        state.select(Some(0));
        terminal
            .draw(|frame| draw(frame, &picker, &mut state, ""))
            .unwrap();
        let highlighted: Vec<_> = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .filter(|cell| cell.fg == MATCH)
            .collect();
        let letters = highlighted
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .concat();
        assert_eq!(letters, "ftlgftlg");
        assert!(
            highlighted
                .iter()
                .all(|cell| cell.modifier.contains(Modifier::BOLD))
        );
        assert!(
            highlighted[..4]
                .iter()
                .all(|cell| cell.bg == Color::Rgb(32, 48, 64))
        );
        assert!(highlighted[4..].iter().all(|cell| cell.bg == Color::Reset));
    }

    #[test]
    fn unicode_match_spans_slice_at_character_boundaries() {
        let spans = branch_name_spans("éclair/été", &[0, 8], Style::default());
        assert_eq!(
            spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<Vec<_>>(),
            vec!["é", "clair/", "é", "té"]
        );
        assert_eq!(spans[0].style.fg, Some(MATCH));
        assert_eq!(spans[2].style.fg, Some(MATCH));
    }
}
