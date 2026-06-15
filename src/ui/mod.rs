pub mod board;
pub mod detail;
pub mod help;
pub mod modals;
pub mod theme;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, GlobalSearch, ToastKind, View};
use crate::forms::Modal;

pub fn draw(app: &App, frame: &mut Frame) {
    if frame.area().width < 2 || frame.area().height < 4 {
        return; // too small to draw anything meaningful (or a 0×0 pty)
    }
    let [header, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(app, frame, header);

    match &app.board {
        None => frame.render_widget(Paragraph::new(" Loading…").style(theme::dim()), body),
        Some(board) => {
            board::draw_board(app, board, frame, body);
            if let View::Detail(detail_state) = &app.view {
                detail::draw_detail(app, board, detail_state, frame, body);
            }
        }
    }

    match &app.modal {
        Some(Modal::TaskForm(form)) => modals::draw_task_form(form, frame, body),
        Some(Modal::Confirm(confirm)) => modals::draw_confirm(confirm, frame, body),
        Some(Modal::Labels(picker)) => {
            if let Some(board) = &app.board {
                modals::draw_label_picker(picker, board, frame, body);
            }
        }
        Some(Modal::Boards(switcher)) => modals::draw_board_switcher(switcher, app, frame, body),
        Some(Modal::Columns(manager)) => {
            if let Some(board) = &app.board {
                modals::draw_column_manager(manager, board, frame, body);
            }
        }
        Some(Modal::Input(input)) => modals::draw_input_modal(input, frame, body),
        Some(Modal::Help) => help::draw_help(frame, body),
        None => {}
    }

    if let Some(search) = &app.search {
        draw_global_search(search, app, frame, body);
    }

    draw_status(app, frame, status);
}

fn draw_header(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    let mut spans = vec![Span::styled(
        " tasko ",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(theme::ACCENT),
    )];
    if let Some(board) = &app.board {
        spans.push(Span::styled("· ", theme::dim()));
        spans.push(Span::styled(
            format!("{} ", board.board.name),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("[{}] ", board.board.key),
            theme::dim(),
        ));
        if app.boards.len() > 1 {
            spans.push(Span::styled(
                format!("· {} boards (b)", app.boards.len()),
                theme::dim(),
            ));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_status(app: &App, frame: &mut Frame, area: ratatui::layout::Rect) {
    if let Some(toast) = &app.toast {
        let style = match toast.kind {
            ToastKind::Error => Style::default()
                .fg(theme::ERROR)
                .add_modifier(Modifier::BOLD),
            ToastKind::Info => Style::default().fg(theme::OK),
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!(" {}", toast.text), style))),
            area,
        );
        return;
    }

    // Filter bar takes over the status line while editing or active.
    if app.filter.editing || app.filter.is_active() {
        let matches: usize = app.board.as_ref().map_or(0, |board| {
            (0..board.columns.len())
                .map(|c| app.visible_count(board, c))
                .sum()
        });
        let mut spans = vec![
            Span::styled(
                " / ",
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(app.filter.raw.clone()),
        ];
        if app.filter.editing {
            spans.push(Span::styled("▏", theme::accent()));
        }
        spans.push(Span::styled(
            format!("  {matches} match(es) · p:<priority> l:<label> · ⏎ keep · Esc clear"),
            theme::dim(),
        ));
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    let mut spans: Vec<Span> = Vec::new();
    if app.pending_saves > 0 {
        spans.push(Span::styled(" ⟳ saving ", theme::accent()));
        spans.push(Span::styled("· ", theme::dim()));
    }
    let hints = match (&app.view, &app.modal) {
        (View::Detail(_), None) => {
            " ↑↓ subtasks · ␣ toggle · n subtask · e edit · l labels · Esc back"
        }
        _ => {
            " ←→↑↓ move · ⏎ open · n new · e edit · d del · ⇧←→ move card · p priority · l labels · / filter · g search · b boards · c columns · ? help · q quit"
        }
    };
    spans.push(Span::styled(hints, theme::dim()));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_global_search(
    search: &GlobalSearch,
    app: &App,
    frame: &mut Frame,
    area: ratatui::layout::Rect,
) {
    use ratatui::widgets::{Block, BorderType, Borders, Clear};

    let height = (search.results.len() as u16).clamp(1, 12) + 5;
    let popup = modals::centered(area, 64, height);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            " Search all boards ",
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [input_area, list_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" 🔍 ", theme::accent()),
            Span::raw(search.input.clone()),
            Span::styled("▏", theme::accent()),
        ])),
        input_area,
    );

    if search.searching {
        frame.render_widget(Paragraph::new(" Searching…").style(theme::dim()), list_area);
    } else if search.results.is_empty() {
        let hint = if search.input.trim().is_empty() {
            " Type to search by key or title"
        } else {
            " No results"
        };
        frame.render_widget(Paragraph::new(hint).style(theme::dim()), list_area);
    } else {
        let visible_rows = list_area.height as usize;
        let offset = search.sel.saturating_sub(visible_rows.saturating_sub(1));
        let lines: Vec<Line> = search
            .results
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_rows)
            .map(|(i, task)| {
                let selected = i == search.sel;
                let board_name = app
                    .boards
                    .iter()
                    .find(|b| b.id == task.board_id)
                    .map(|b| b.name.as_str())
                    .unwrap_or("?");
                let mut style = Style::default();
                if selected {
                    style = style.bg(ratatui::style::Color::Indexed(236));
                }
                Line::from(vec![
                    Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
                    Span::styled(format!("{} ", task.key), style.fg(theme::DIM)),
                    Span::styled(task.title.as_str(), style),
                    Span::styled(format!("  · {board_name}"), style.fg(theme::DIM)),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), list_area);
    }

    frame.render_widget(
        Paragraph::new(Span::styled(
            " ⏎ jump to task · ↑↓ select · Esc close",
            theme::dim(),
        )),
        hint_area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::{App, DbResult, Message, channel};
    use crate::db::Db;
    use crate::state::bootstrap;

    async fn app_with_seed(count: usize) -> App {
        let db = Db::connect_in_memory().await.unwrap();
        crate::seed::seed(&db, count).await.unwrap();
        let (tx, _rx) = channel();
        let mut app = App::new(db.clone(), tx);
        let (boards, state) = bootstrap(&db).await.unwrap();
        app.update(Message::Db(DbResult::Bootstrapped(boards, Box::new(state))));
        app
    }

    fn render_to_string(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| super::draw(app, frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer.content().iter().map(|cell| cell.symbol()).collect()
    }

    #[tokio::test]
    async fn board_renders_columns_and_cards() {
        let app = app_with_seed(9).await;
        let content = render_to_string(&app, 120, 36);
        assert!(content.contains("To Do"));
        assert!(content.contains("In Progress"));
        assert!(content.contains("Done"));
        assert!(content.contains("MB-1"));
        assert!(content.contains("tasko"));
    }

    #[tokio::test]
    async fn help_overlay_renders() {
        let mut app = app_with_seed(1).await;
        app.update(Message::OpenHelp);
        let content = render_to_string(&app, 120, 40);
        assert!(content.contains("Help"));
        assert!(content.contains("Navigate cards"));
    }

    #[tokio::test]
    async fn detail_view_renders() {
        let mut app = app_with_seed(7).await;
        app.update(Message::OpenDetail);
        let content = render_to_string(&app, 120, 40);
        assert!(content.contains("Description"));
        assert!(content.contains("Activity"));
    }

    #[tokio::test]
    async fn filter_bar_shows_matches() {
        let mut app = app_with_seed(9).await;
        app.update(Message::FilterStart);
        for c in "login".chars() {
            app.update(Message::FilterKey(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(c),
                crossterm::event::KeyModifiers::NONE,
            )));
        }
        let content = render_to_string(&app, 120, 36);
        assert!(content.contains("match(es)"));
    }

    #[tokio::test]
    async fn tiny_terminal_does_not_panic() {
        let app = app_with_seed(3).await;
        for (w, h) in [(1, 1), (2, 2), (10, 3), (25, 5)] {
            let _ = render_to_string(&app, w, h);
        }
    }
}
