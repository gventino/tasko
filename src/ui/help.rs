use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::ui::modals::centered;
use crate::ui::theme;

const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Board",
        &[
            ("←→↑↓ / Tab", "Navigate cards and columns"),
            ("Home / End", "Jump to top / bottom of column"),
            ("⏎", "Open task detail"),
            ("n", "New task in current column"),
            ("e", "Edit selected task"),
            ("d", "Delete selected task"),
            ("⇧←→", "Move card across columns"),
            ("⇧↑↓", "Reorder card in column"),
            ("p", "Cycle priority"),
            ("l", "Edit labels of task"),
        ],
    ),
    (
        "Navigation & tools",
        &[
            ("/", "Filter board (text, p:<priority>, l:<label>)"),
            ("g", "Search across all boards"),
            ("b", "Board switcher (n new · r rename · d delete)"),
            ("c", "Column manager (n/r/d · w wip limit · ⇧↑↓ reorder)"),
            ("R", "Reload board from disk"),
            ("q / Ctrl+C", "Quit"),
        ],
    ),
    (
        "Task detail",
        &[
            ("↑↓", "Select subtask"),
            ("␣ / x", "Toggle subtask done"),
            ("n", "Add subtask"),
            ("E / D", "Edit / delete selected subtask"),
            ("e · p · l", "Edit / priority / labels of open task"),
            ("Esc", "Back to board"),
        ],
    ),
];

pub fn draw_help(frame: &mut Frame, area: Rect) {
    let rows: u16 = SECTIONS
        .iter()
        .map(|(_, items)| items.len() as u16 + 2)
        .sum::<u16>()
        + 3;
    let popup = centered(area, 64, rows.min(area.height));
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            " Help ",
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [list_area, hint_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    let mut lines: Vec<Line> = Vec::new();
    for (section, items) in SECTIONS {
        lines.push(Line::from(Span::styled(
            format!(" {section}"),
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )));
        for (keys, what) in *items {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("   {keys:<12}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled((*what).to_string(), theme::dim()),
            ]));
        }
        lines.push(Line::raw(""));
    }
    frame.render_widget(Paragraph::new(lines), list_area);
    frame.render_widget(
        Paragraph::new(Span::styled(" Esc / ? close", theme::dim())),
        hint_area,
    );
}
