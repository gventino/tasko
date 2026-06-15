use chrono::{Datelike, Local};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::app::App;
use crate::domain::Task;
use crate::state::BoardState;
use crate::ui::theme;

/// Card height in rows: key/priority line, title line, meta line, separator.
const CARD_H: usize = 4;
const MIN_COL_W: u16 = 26;
const MAX_COL_W: u16 = 46;
const SELECTED_BG: Color = Color::Indexed(236);

pub fn draw_board(app: &App, board: &BoardState, frame: &mut Frame, area: Rect) {
    if area.width < 2 || area.height < 2 {
        return;
    }
    if board.columns.is_empty() {
        let empty = Paragraph::new("No columns yet — press c to create one").style(theme::dim());
        frame.render_widget(empty, area.inner(Margin::new(2, 1)));
        return;
    }

    let today = Local::now().date_naive();
    let n_cols = board.columns.len();
    let fit = ((area.width / MIN_COL_W).max(1) as usize).min(n_cols);
    let col_w = (area.width / fit as u16).clamp(MIN_COL_W, MAX_COL_W);

    // Keep the selected column inside the visible window.
    let first = app.sel_col.saturating_sub(fit.saturating_sub(1));
    let visible = &board.columns[first..(first + fit).min(n_cols)];

    let constraints: Vec<Constraint> = visible.iter().map(|_| Constraint::Length(col_w)).collect();
    let chunks = Layout::horizontal(constraints).split(area);

    for (i, column) in visible.iter().enumerate() {
        let col_idx = first + i;
        draw_column(app, board, col_idx, column, today, frame, chunks[i]);
    }

    // Hint arrows when columns overflow on either side.
    if first > 0 {
        frame.render_widget(
            Paragraph::new("◀").style(theme::accent()),
            Rect {
                x: area.x,
                y: area.y,
                width: 1,
                height: 1,
            },
        );
    }
    if first + fit < n_cols {
        frame.render_widget(
            Paragraph::new("▶").style(theme::accent()),
            Rect {
                x: area.right().saturating_sub(1),
                y: area.y,
                width: 1,
                height: 1,
            },
        );
    }
}

fn draw_column(
    app: &App,
    board: &BoardState,
    col_idx: usize,
    column: &crate::domain::Column,
    today: chrono::NaiveDate,
    frame: &mut Frame,
    area: Rect,
) {
    let tasks = app.visible_tasks(board, col_idx);
    let count = tasks.len();
    let is_selected_col = col_idx == app.sel_col;

    let mut title_spans = vec![Span::styled(
        format!(" {} ", column.name),
        if is_selected_col {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        },
    )];
    let over_wip = column.wip_limit.is_some_and(|limit| (count as i64) > limit);
    let count_text = match column.wip_limit {
        Some(limit) => format!("{count}/{limit} "),
        None => format!("{count} "),
    };
    title_spans.push(Span::styled(
        count_text,
        if over_wip {
            Style::default()
                .fg(theme::ERROR)
                .add_modifier(Modifier::BOLD)
        } else {
            theme::dim()
        },
    ));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if is_selected_col {
            BorderType::Thick
        } else {
            BorderType::Rounded
        })
        .border_style(if is_selected_col {
            theme::selected_border()
        } else {
            theme::dim()
        })
        .title(Line::from(title_spans));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if tasks.is_empty() {
        let hint = if app.filter_active() {
            "no matches"
        } else {
            "empty — n adds a task"
        };
        frame.render_widget(
            Paragraph::new(hint).style(theme::dim()),
            inner.inner(Margin::new(1, 1)),
        );
        return;
    }

    let rows_fit = (inner.height as usize / CARD_H).max(1);
    let sel_row = if is_selected_col {
        app.sel_row.min(count - 1)
    } else {
        0
    };
    let offset = if is_selected_col {
        sel_row.saturating_sub(rows_fit - 1)
    } else {
        0
    };

    for (slot, task) in tasks.iter().skip(offset).take(rows_fit).enumerate() {
        let y = inner.y + (slot * CARD_H) as u16;
        let card_area = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: (CARD_H as u16 - 1).min(inner.bottom().saturating_sub(y)),
        };
        if card_area.height == 0 {
            break;
        }
        let selected = is_selected_col && offset + slot == sel_row;
        draw_card(board, task, selected, today, frame, card_area);
    }

    if count > rows_fit {
        let mut sb_state = ScrollbarState::new(count.saturating_sub(rows_fit)).position(offset);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .style(theme::dim())
                .begin_symbol(None)
                .end_symbol(None),
            inner,
            &mut sb_state,
        );
    }
}

fn draw_card(
    board: &BoardState,
    task: &Task,
    selected: bool,
    today: chrono::NaiveDate,
    frame: &mut Frame,
    area: Rect,
) {
    let base = if selected {
        Style::default().bg(SELECTED_BG)
    } else {
        Style::default()
    };
    let pri_color = theme::priority_color(task.priority);

    // Line 1: key + priority icon.
    let mut head = vec![
        Span::styled(if selected { "▶ " } else { "  " }, base.fg(theme::ACCENT)),
        Span::styled(task.key.as_str(), base.fg(theme::DIM)),
        Span::raw(" "),
        Span::styled(task.priority.icon(), base.fg(pri_color)),
    ];
    if let Some((done, total)) = board.subtask_progress(task.id) {
        head.push(Span::styled(
            format!("  ☰ {done}/{total}"),
            base.fg(if done == total { theme::OK } else { theme::DIM }),
        ));
    }

    // Line 2: title.
    let title_style = if selected {
        base.add_modifier(Modifier::BOLD)
    } else {
        base
    };
    let title = Line::from(vec![
        Span::raw("  "),
        Span::styled(task.title.as_str(), title_style),
    ]);

    // Line 3: labels + due date.
    let mut meta = vec![Span::raw("  ")];
    if let Some(label_ids) = board.task_labels.get(&task.id) {
        for label in board.labels.iter().filter(|l| label_ids.contains(&l.id)) {
            meta.push(Span::styled("● ", base.fg(theme::label_color(label.color))));
        }
    }
    if let Some(due) = task.due_date {
        let overdue = task.is_overdue(today);
        let text = if due.year() == today.year() {
            due.format("%b %d").to_string()
        } else {
            due.format("%Y %b %d").to_string()
        };
        meta.push(Span::styled(
            format!("⏱ {text}"),
            if overdue {
                base.fg(theme::ERROR).add_modifier(Modifier::BOLD)
            } else {
                base.fg(theme::DIM)
            },
        ));
    }

    let lines: Vec<Line> = vec![
        Line::from(head).style(base),
        title.style(base),
        Line::from(meta).style(base),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}
