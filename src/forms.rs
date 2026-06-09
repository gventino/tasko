use chrono::NaiveDate;
use ratatui::style::{Modifier, Style};
use tui_textarea::TextArea;

use crate::domain::{Id, Priority, Task};

pub enum Modal {
    TaskForm(Box<TaskForm>),
    Confirm(Confirm),
    Labels(LabelPicker),
    Boards(BoardSwitcher),
    Columns(ColumnManager),
    Input(InputModal),
    Help,
}

pub struct BoardSwitcher {
    pub sel: usize,
}

pub struct ColumnManager {
    pub sel: usize,
}

/// Generic one-line input modal (column names, board names, WIP limits…).
pub struct InputModal {
    pub title: String,
    pub action: InputAction,
    pub textarea: TextArea<'static>,
    pub error: Option<String>,
}

#[derive(Clone, Copy)]
pub enum InputAction {
    NewColumn,
    RenameColumn(Id),
    NewBoard,
    RenameBoard(Id),
    WipLimit(Id),
}

impl InputModal {
    pub fn new(title: &str, action: InputAction, initial: &str, placeholder: &str) -> Self {
        Self {
            title: title.to_string(),
            action,
            textarea: text_field(initial, placeholder),
            error: None,
        }
    }

    pub fn value(&self) -> String {
        self.textarea.lines().join(" ").trim().to_string()
    }
}

pub struct LabelPicker {
    pub task_id: Id,
    pub sel: usize,
    pub adding: Option<TextArea<'static>>,
}

impl LabelPicker {
    pub fn new(task_id: Id) -> Self {
        Self {
            task_id,
            sel: 0,
            adding: None,
        }
    }

    pub fn start_adding(&mut self) {
        self.adding = Some(text_field("", "Label name…"));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FormFocus {
    Title,
    Description,
    Priority,
    Due,
}

impl FormFocus {
    pub fn next(self) -> Self {
        match self {
            FormFocus::Title => FormFocus::Description,
            FormFocus::Description => FormFocus::Priority,
            FormFocus::Priority => FormFocus::Due,
            FormFocus::Due => FormFocus::Title,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            FormFocus::Title => FormFocus::Due,
            FormFocus::Description => FormFocus::Title,
            FormFocus::Priority => FormFocus::Description,
            FormFocus::Due => FormFocus::Priority,
        }
    }
}

pub struct TaskForm {
    pub editing: Option<Id>,
    pub column_id: Id,
    pub parent_id: Option<Id>,
    pub title: TextArea<'static>,
    pub description: TextArea<'static>,
    pub due: TextArea<'static>,
    pub priority: Priority,
    pub focus: FormFocus,
    pub error: Option<String>,
}

pub struct FormOutput {
    pub title: String,
    pub description: String,
    pub priority: Priority,
    pub due_date: Option<NaiveDate>,
}

fn text_field(text: &str, placeholder: &str) -> TextArea<'static> {
    let mut area = if text.is_empty() {
        TextArea::default()
    } else {
        TextArea::from(text.split('\n').map(str::to_string).collect::<Vec<_>>())
    };
    area.set_cursor_line_style(Style::default());
    area.set_placeholder_text(placeholder);
    area.set_placeholder_style(Style::default().fg(ratatui::style::Color::DarkGray));
    area.move_cursor(tui_textarea::CursorMove::End);
    area
}

impl TaskForm {
    pub fn create_in(column_id: Id, parent_id: Option<Id>) -> Self {
        Self {
            editing: None,
            column_id,
            parent_id,
            title: text_field("", "Task title…"),
            description: text_field("", "Description (optional)"),
            due: text_field("", "YYYY-MM-DD (optional)"),
            priority: Priority::Medium,
            focus: FormFocus::Title,
            error: None,
        }
    }

    pub fn edit(task: &Task) -> Self {
        Self {
            editing: Some(task.id),
            column_id: task.column_id,
            parent_id: task.parent_id,
            title: text_field(&task.title, "Task title…"),
            description: text_field(&task.description, "Description (optional)"),
            due: text_field(
                &task.due_date.map(|d| d.to_string()).unwrap_or_default(),
                "YYYY-MM-DD (optional)",
            ),
            priority: task.priority,
            focus: FormFocus::Title,
            error: None,
        }
    }

    pub fn focused_textarea_mut(&mut self) -> Option<&mut TextArea<'static>> {
        match self.focus {
            FormFocus::Title => Some(&mut self.title),
            FormFocus::Description => Some(&mut self.description),
            FormFocus::Due => Some(&mut self.due),
            FormFocus::Priority => None,
        }
    }

    pub fn validate(&self) -> Result<FormOutput, String> {
        let title = self.title.lines().join(" ").trim().to_string();
        if title.is_empty() {
            return Err("Title cannot be empty".into());
        }
        let due_text = self.due.lines().join("").trim().to_string();
        let due_date = if due_text.is_empty() {
            None
        } else {
            Some(
                NaiveDate::parse_from_str(&due_text, "%Y-%m-%d")
                    .map_err(|_| format!("Invalid date '{due_text}' — use YYYY-MM-DD"))?,
            )
        };
        let description = self.description.lines().join("\n").trim_end().to_string();
        Ok(FormOutput {
            title,
            description,
            priority: self.priority,
            due_date,
        })
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }
}

pub struct Confirm {
    pub text: String,
    pub action: ConfirmAction,
}

#[derive(Clone, Copy)]
#[allow(clippy::enum_variant_names)] // "Delete" prefix is the whole point
pub enum ConfirmAction {
    DeleteTask(Id),
    DeleteLabel(Id),
    DeleteBoard(Id),
    DeleteColumn(Id),
}

pub fn focused_block_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(ratatui::style::Color::DarkGray)
    }
}
