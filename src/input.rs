use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Message, View};
use crate::forms::{FormFocus, LabelPicker, Modal, TaskForm};

/// Translate a key press into a semantic message, given the current app state.
/// Returns `None` when the key has no meaning in this context.
pub fn map_key(app: &App, key: KeyEvent) -> Option<Message> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Message::Quit);
    }
    if app.search.is_some() {
        return map_global_search_key(key);
    }
    if app.filter.editing {
        return map_filter_key(key);
    }
    match &app.modal {
        Some(Modal::TaskForm(form)) => map_form_key(form, key),
        Some(Modal::Confirm(_)) => map_confirm_key(key),
        Some(Modal::Labels(picker)) => map_labels_key(picker, key),
        Some(Modal::Boards(_)) => map_boards_key(key),
        Some(Modal::Columns(_)) => map_columns_key(key),
        Some(Modal::Input(_)) => map_input_modal_key(key),
        Some(Modal::Help) => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => Some(Message::CloseModal),
            _ => None,
        },
        None => match &app.view {
            View::Board => map_board_key(app, key),
            View::Detail(_) => map_detail_key(key),
        },
    }
}

fn map_filter_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc => Some(Message::FilterClear),
        KeyCode::Enter => Some(Message::FilterConfirm),
        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
            // Allow navigating while the filter bar is open.
            Some(Message::FilterConfirm)
        }
        _ => Some(Message::FilterKey(key)),
    }
}

fn map_global_search_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc => Some(Message::CloseModal),
        KeyCode::Enter => Some(Message::GlobalSearchOpen),
        KeyCode::Up => Some(Message::GlobalSearchUp),
        KeyCode::Down => Some(Message::GlobalSearchDown),
        _ => Some(Message::GlobalSearchKey(key)),
    }
}

fn map_boards_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('b') => Some(Message::CloseModal),
        KeyCode::Up => Some(Message::BoardUp),
        KeyCode::Down => Some(Message::BoardDown),
        KeyCode::Enter => Some(Message::SwitchBoard),
        KeyCode::Char('n') => Some(Message::NewBoardStart),
        KeyCode::Char('r') => Some(Message::RenameBoardStart),
        KeyCode::Char('d') => Some(Message::DeleteBoardStart),
        _ => None,
    }
}

fn map_columns_key(key: KeyEvent) -> Option<Message> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('c') => Some(Message::CloseModal),
        KeyCode::Up if shift => Some(Message::ColMoveUp),
        KeyCode::Down if shift => Some(Message::ColMoveDown),
        KeyCode::Up => Some(Message::ColUp),
        KeyCode::Down => Some(Message::ColDown),
        KeyCode::Char('n') => Some(Message::NewColumnStart),
        KeyCode::Char('r') => Some(Message::RenameColumnStart),
        KeyCode::Char('d') => Some(Message::DeleteColumnStart),
        KeyCode::Char('w') => Some(Message::WipLimitStart),
        _ => None,
    }
}

fn map_input_modal_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc => Some(Message::CloseModal),
        KeyCode::Enter => Some(Message::InputModalSubmit),
        _ => Some(Message::InputModalKey(key)),
    }
}

fn map_labels_key(picker: &LabelPicker, key: KeyEvent) -> Option<Message> {
    if picker.adding.is_some() {
        return match key.code {
            KeyCode::Esc => Some(Message::NewLabelCancel),
            KeyCode::Enter => Some(Message::NewLabelSubmit),
            _ => Some(Message::NewLabelInput(key)),
        };
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => Some(Message::CloseModal),
        KeyCode::Up => Some(Message::LabelUp),
        KeyCode::Down => Some(Message::LabelDown),
        KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Char('x') => Some(Message::ToggleLabel),
        KeyCode::Char('n') => Some(Message::NewLabelStart),
        KeyCode::Char('d') => Some(Message::DeleteLabel),
        _ => None,
    }
}

fn map_board_key(app: &App, key: KeyEvent) -> Option<Message> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Esc if app.filter.is_active() => Some(Message::FilterClear),
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Left if shift => Some(Message::MoveTaskLeft),
        KeyCode::Right if shift => Some(Message::MoveTaskRight),
        KeyCode::Up if shift => Some(Message::MoveTaskUp),
        KeyCode::Down if shift => Some(Message::MoveTaskDown),
        KeyCode::Left => Some(Message::SelectLeft),
        KeyCode::Right => Some(Message::SelectRight),
        KeyCode::Up => Some(Message::SelectUp),
        KeyCode::Down => Some(Message::SelectDown),
        KeyCode::Tab => Some(Message::SelectRight),
        KeyCode::BackTab => Some(Message::SelectLeft),
        KeyCode::Home => Some(Message::SelectTop),
        KeyCode::End => Some(Message::SelectBottom),
        KeyCode::Char('n') => Some(Message::OpenNewTask),
        KeyCode::Char('e') => Some(Message::OpenEditTask),
        KeyCode::Char('d') => Some(Message::OpenDeleteConfirm),
        KeyCode::Char('p') => Some(Message::CyclePriority),
        KeyCode::Char('l') => Some(Message::OpenLabelPicker),
        KeyCode::Char('b') => Some(Message::OpenBoardSwitcher),
        KeyCode::Char('c') => Some(Message::OpenColumnManager),
        KeyCode::Char('/') => Some(Message::FilterStart),
        KeyCode::Char('g') => Some(Message::OpenGlobalSearch),
        KeyCode::Char('?') => Some(Message::OpenHelp),
        KeyCode::Enter => Some(Message::OpenDetail),
        KeyCode::Char('R') => Some(Message::ReloadBoard),
        _ => None,
    }
}

fn map_detail_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::CloseDetail),
        KeyCode::Up => Some(Message::DetailUp),
        KeyCode::Down => Some(Message::DetailDown),
        KeyCode::Char(' ') | KeyCode::Char('x') => Some(Message::ToggleSubtask),
        KeyCode::Char('n') => Some(Message::NewSubtask),
        KeyCode::Char('e') => Some(Message::OpenEditTask),
        KeyCode::Char('E') => Some(Message::EditSubtask),
        KeyCode::Char('D') => Some(Message::DeleteSubtask),
        KeyCode::Char('p') => Some(Message::CyclePriority),
        KeyCode::Char('l') => Some(Message::OpenLabelPicker),
        KeyCode::Char('?') => Some(Message::OpenHelp),
        _ => None,
    }
}

fn map_form_key(form: &TaskForm, key: KeyEvent) -> Option<Message> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => Some(Message::CloseModal),
        KeyCode::Char('s') if ctrl => Some(Message::FormSubmit),
        KeyCode::Tab => Some(Message::FormNextField),
        KeyCode::BackTab => Some(Message::FormPrevField),
        KeyCode::Enter if form.focus != FormFocus::Description => Some(Message::FormSubmit),
        KeyCode::Left | KeyCode::Up if form.focus == FormFocus::Priority => {
            Some(Message::FormCyclePriority(-1))
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char(' ')
            if form.focus == FormFocus::Priority =>
        {
            Some(Message::FormCyclePriority(1))
        }
        _ => Some(Message::FormInput(key)),
    }
}

fn map_confirm_key(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Message::ConfirmYes),
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => Some(Message::CloseModal),
        _ => None,
    }
}
