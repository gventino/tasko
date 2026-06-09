use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use crate::app::App;
use crate::domain::Priority;
use crate::forms::{
    BoardSwitcher, ColumnManager, Confirm, FormFocus, InputModal, LabelPicker, TaskForm,
    focused_block_style,
};
use crate::state::BoardState;
use crate::ui::theme;

/// Center a `width`×`height` rect inside `area` (clamped to fit).
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [h] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [rect] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(h);
    rect
}

pub fn draw_task_form(form: &TaskForm, frame: &mut Frame, area: Rect) {
    let popup = centered(area, 64, 19);
    frame.render_widget(Clear, popup);

    let title = if form.is_editing() {
        " Edit Task "
    } else {
        " New Task "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [
        title_area,
        desc_area,
        priority_area,
        due_area,
        error_area,
        hint_area,
    ] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(6),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    draw_field(
        frame,
        &form.title,
        "Title",
        form.focus == FormFocus::Title,
        title_area,
    );
    draw_field(
        frame,
        &form.description,
        "Description",
        form.focus == FormFocus::Description,
        desc_area,
    );
    draw_priority(
        frame,
        form.priority,
        form.focus == FormFocus::Priority,
        priority_area,
    );
    draw_field(
        frame,
        &form.due,
        "Due date",
        form.focus == FormFocus::Due,
        due_area,
    );

    if let Some(error) = &form.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" ✗ {error}"),
                Style::default().fg(theme::ERROR),
            )),
            error_area,
        );
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            " Tab next field · ⏎ save · Ctrl+S save · Esc cancel",
            theme::dim(),
        )),
        hint_area,
    );
}

fn draw_field(
    frame: &mut Frame,
    textarea: &tui_textarea::TextArea<'static>,
    label: &str,
    focused: bool,
    area: Rect,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focused_block_style(focused))
        .title(label.to_string());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if focused {
        frame.render_widget(textarea, inner);
    } else {
        // Render a plain snapshot so the inactive cursor is not shown.
        let text = textarea.lines().join("\n");
        if text.is_empty() {
            frame.render_widget(
                Paragraph::new(textarea.placeholder_text().to_string()).style(theme::dim()),
                inner,
            );
        } else {
            frame.render_widget(Paragraph::new(text), inner);
        }
    }
}

fn draw_priority(frame: &mut Frame, priority: Priority, focused: bool, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focused_block_style(focused))
        .title("Priority");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans = vec![Span::styled(
        if focused { "◀ " } else { "  " },
        theme::dim(),
    )];
    spans.push(Span::styled(
        format!("{} {}", priority.icon(), priority.name()),
        Style::default()
            .fg(theme::priority_color(priority))
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(if focused { " ▶" } else { "" }, theme::dim()));
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

pub fn draw_label_picker(picker: &LabelPicker, board: &BoardState, frame: &mut Frame, area: Rect) {
    let task_key = board
        .find_task(picker.task_id)
        .map(|t| t.key.clone())
        .unwrap_or_default();
    let rows = (board.labels.len() as u16).clamp(1, 10);
    let adding_extra = if picker.adding.is_some() { 3 } else { 0 };
    let popup = centered(area, 44, rows + 5 + adding_extra);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            format!(" Labels · {task_key} "),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [list_area, add_area, hint_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(adding_extra),
        Constraint::Length(1),
    ])
    .areas(inner);

    if board.labels.is_empty() {
        frame.render_widget(
            Paragraph::new("No labels on this board yet — press n").style(theme::dim()),
            list_area,
        );
    } else {
        let assigned = board.task_labels.get(&picker.task_id);
        let visible_rows = list_area.height as usize;
        let offset = picker.sel.saturating_sub(visible_rows.saturating_sub(1));
        let lines: Vec<Line> = board
            .labels
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_rows)
            .map(|(i, label)| {
                let selected = i == picker.sel;
                let has = assigned.is_some_and(|ids| ids.contains(&label.id));
                let mut style = Style::default();
                if selected {
                    style = style.bg(ratatui::style::Color::Indexed(236));
                }
                Line::from(vec![
                    Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
                    Span::styled(if has { "☑ " } else { "☐ " }, style),
                    Span::styled("⬤ ", style.fg(theme::label_color(label.color))),
                    Span::styled(label.name.clone(), style),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), list_area);
    }

    if let Some(textarea) = &picker.adding {
        let add_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme::accent())
            .title("New label");
        let add_inner = add_block.inner(add_area);
        frame.render_widget(add_block, add_area);
        frame.render_widget(textarea, add_inner);
    }

    let hint = if picker.adding.is_some() {
        " ⏎ create · Esc cancel"
    } else {
        " ␣/⏎ toggle · n new · d delete · Esc close"
    };
    frame.render_widget(Paragraph::new(Span::styled(hint, theme::dim())), hint_area);
}

pub fn draw_board_switcher(switcher: &BoardSwitcher, app: &App, frame: &mut Frame, area: Rect) {
    let rows = (app.boards.len() as u16).clamp(1, 12);
    let popup = centered(area, 48, rows + 4);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            " Boards ",
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [list_area, hint_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    let active_id = app.board.as_ref().map(|b| b.board.id);
    let visible_rows = list_area.height as usize;
    let offset = switcher.sel.saturating_sub(visible_rows.saturating_sub(1));
    let lines: Vec<Line> = app
        .boards
        .iter()
        .enumerate()
        .skip(offset)
        .take(visible_rows)
        .map(|(i, board)| {
            let selected = i == switcher.sel;
            let active = active_id == Some(board.id);
            let mut style = Style::default();
            if selected {
                style = style.bg(ratatui::style::Color::Indexed(236));
            }
            Line::from(vec![
                Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
                Span::styled(
                    format!("{} ", board.name),
                    style.add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("[{}]", board.key), style.fg(theme::DIM)),
                Span::styled(if active { "  ● active" } else { "" }, style.fg(theme::OK)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list_area);
    frame.render_widget(
        Paragraph::new(Span::styled(
            " ⏎ open · n new · r rename · d delete · Esc close",
            theme::dim(),
        )),
        hint_area,
    );
}

pub fn draw_column_manager(
    manager: &ColumnManager,
    board: &BoardState,
    frame: &mut Frame,
    area: Rect,
) {
    let rows = (board.columns.len() as u16).clamp(1, 12);
    let popup = centered(area, 52, rows + 4);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            " Columns ",
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [list_area, hint_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    if board.columns.is_empty() {
        frame.render_widget(
            Paragraph::new("No columns — press n to create one").style(theme::dim()),
            list_area,
        );
    } else {
        let visible_rows = list_area.height as usize;
        let offset = manager.sel.saturating_sub(visible_rows.saturating_sub(1));
        let lines: Vec<Line> = board
            .columns
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_rows)
            .map(|(i, column)| {
                let selected = i == manager.sel;
                let mut style = Style::default();
                if selected {
                    style = style.bg(ratatui::style::Color::Indexed(236));
                }
                let count = board.tasks[i].len();
                let wip = column
                    .wip_limit
                    .map(|w| format!("  wip {w}"))
                    .unwrap_or_default();
                Line::from(vec![
                    Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
                    Span::styled(
                        format!("{} ", column.name),
                        style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("({count}){wip}"), style.fg(theme::DIM)),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), list_area);
    }
    frame.render_widget(
        Paragraph::new(Span::styled(
            " ⇧↑↓ reorder · n new · r rename · w wip · d delete · Esc",
            theme::dim(),
        )),
        hint_area,
    );
}

pub fn draw_input_modal(modal: &InputModal, frame: &mut Frame, area: Rect) {
    let popup = centered(area, 50, 7);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            format!(" {} ", modal.title),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [field_area, error_area, hint_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let field_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(focused_block_style(true));
    let field_inner = field_block.inner(field_area);
    frame.render_widget(field_block, field_area);
    frame.render_widget(&modal.textarea, field_inner);

    if let Some(error) = &modal.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" ✗ {error}"),
                Style::default().fg(theme::ERROR),
            )),
            error_area,
        );
    }
    frame.render_widget(
        Paragraph::new(Span::styled(" ⏎ confirm · Esc cancel", theme::dim())),
        hint_area,
    );
}

pub fn draw_confirm(confirm: &Confirm, frame: &mut Frame, area: Rect) {
    let width = 56.min(area.width.saturating_sub(4)).max(20);
    let popup = centered(area, width, 7);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::ERROR))
        .title(Span::styled(
            " Confirm ",
            Style::default()
                .fg(theme::ERROR)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let [text_area, _, hint_area] = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);
    frame.render_widget(
        Paragraph::new(confirm.text.clone()).wrap(Wrap { trim: true }),
        text_area,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(" y/⏎ confirm · n/Esc cancel", theme::dim())),
        hint_area,
    );
}
