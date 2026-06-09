use chrono::Local;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};

use crate::app::{App, DetailState};
use crate::state::BoardState;
use crate::ui::modals::centered;
use crate::ui::theme;

pub fn draw_detail(
    app: &App,
    board: &BoardState,
    detail: &DetailState,
    frame: &mut Frame,
    area: Rect,
) {
    let Some(task) = board.find_task(detail.task_id) else {
        return;
    };

    let width = (area.width * 4 / 5).clamp(40, 100);
    let height = (area.height * 9 / 10).max(16).min(area.height);
    let popup = centered(area, width, height);
    frame.render_widget(Clear, popup);

    let column_name = board
        .column_index(task.column_id)
        .map(|i| board.columns[i].name.as_str())
        .unwrap_or("?");
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::accent())
        .title(Span::styled(
            format!(" {} · {} ", task.key, column_name),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let subs = board
        .subtasks
        .get(&task.id)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let subs_height = (subs.len() as u16).clamp(1, 6) + 1;

    let [
        title_area,
        meta_area,
        desc_area,
        subs_area,
        acts_area,
        hint_area,
    ] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Fill(2),
        Constraint::Length(subs_height + 1),
        Constraint::Fill(3),
        Constraint::Length(1),
    ])
    .areas(inner);

    // Title.
    frame.render_widget(
        Paragraph::new(Span::styled(
            task.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ))
        .wrap(Wrap { trim: true }),
        title_area,
    );

    // Meta line: priority, labels, due, timestamps.
    let mut meta = vec![
        Span::styled(
            format!("{} {}", task.priority.icon(), task.priority.name()),
            Style::default().fg(theme::priority_color(task.priority)),
        ),
        Span::styled("  ", theme::dim()),
    ];
    if let Some(label_ids) = board.task_labels.get(&task.id) {
        for label in board.labels.iter().filter(|l| label_ids.contains(&l.id)) {
            meta.push(Span::styled(
                format!("⬤ {} ", label.name),
                Style::default().fg(theme::label_color(label.color)),
            ));
        }
        if !label_ids.is_empty() {
            meta.push(Span::styled(" ", theme::dim()));
        }
    }
    if let Some(due) = task.due_date {
        let overdue = due < Local::now().date_naive();
        meta.push(Span::styled(
            format!("⏱ due {due}  "),
            if overdue {
                Style::default()
                    .fg(theme::ERROR)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::dim()
            },
        ));
    }
    meta.push(Span::styled(
        format!(
            "created {}  updated {}",
            task.created_at
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M"),
            task.updated_at
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M"),
        ),
        theme::dim(),
    ));
    frame.render_widget(Paragraph::new(Line::from(meta)), meta_area);

    // Description.
    let desc_block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme::dim())
        .title(Span::styled("Description", theme::dim()));
    let desc_inner = desc_block.inner(desc_area);
    frame.render_widget(desc_block, desc_area);
    if task.description.is_empty() {
        frame.render_widget(
            Paragraph::new("No description").style(theme::dim()),
            desc_inner,
        );
    } else {
        frame.render_widget(
            Paragraph::new(task.description.clone()).wrap(Wrap { trim: false }),
            desc_inner,
        );
    }

    // Subtasks.
    let progress = board
        .subtask_progress(task.id)
        .map(|(done, total)| format!("Subtasks {done}/{total}"))
        .unwrap_or_else(|| "Subtasks".to_string());
    let subs_block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme::dim())
        .title(Span::styled(progress, theme::dim()));
    let subs_inner = subs_block.inner(subs_area);
    frame.render_widget(subs_block, subs_area);

    if subs.is_empty() {
        frame.render_widget(
            Paragraph::new("No subtasks — press n to add one").style(theme::dim()),
            subs_inner,
        );
    } else {
        let visible_rows = subs_inner.height as usize;
        let offset = detail
            .sel_sub
            .saturating_sub(visible_rows.saturating_sub(1));
        let lines: Vec<Line> = subs
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible_rows)
            .map(|(i, sub)| {
                let selected = i == detail.sel_sub;
                let check = if sub.done { "☑" } else { "☐" };
                let mut style = if sub.done {
                    Style::default()
                        .fg(theme::DIM)
                        .add_modifier(Modifier::CROSSED_OUT)
                } else {
                    Style::default()
                };
                if selected {
                    style = style.bg(Color::Indexed(236)).add_modifier(Modifier::BOLD);
                }
                Line::from(vec![
                    Span::styled(if selected { "▶ " } else { "  " }, theme::accent()),
                    Span::styled(format!("{check} {}", sub.title), style),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), subs_inner);
    }

    // Activity log.
    let acts_block = Block::default()
        .borders(Borders::TOP)
        .border_style(theme::dim())
        .title(Span::styled("Activity", theme::dim()));
    let acts_inner = acts_block.inner(acts_area);
    frame.render_widget(acts_block, acts_area);

    if !detail.activities_loaded {
        frame.render_widget(Paragraph::new("Loading…").style(theme::dim()), acts_inner);
    } else if detail.activities.is_empty() {
        frame.render_widget(
            Paragraph::new("No activity yet").style(theme::dim()),
            acts_inner,
        );
    } else {
        let lines: Vec<Line> = detail
            .activities
            .iter()
            .take(acts_inner.height as usize)
            .map(|act| {
                Line::from(vec![
                    Span::styled(
                        format!(
                            "{} ",
                            act.created_at.with_timezone(&Local).format("%m-%d %H:%M")
                        ),
                        theme::dim(),
                    ),
                    Span::raw(act.detail.clone()),
                ])
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), acts_inner);
    }

    let _ = app;
    frame.render_widget(
        Paragraph::new(Span::styled(
            "↑↓ subtasks · ␣ toggle · n add · E edit sub · D delete sub · e edit task · p priority · Esc close",
            theme::dim(),
        )),
        hint_area,
    );
}
