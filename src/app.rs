use std::future::Future;

use anyhow::Result;
use crossterm::event::KeyEvent;
use tokio::sync::mpsc;

use crate::db::Db;
use crate::db::tasks::NewTask;
use crate::domain::{Activity, Board, Id, Task, activity_kind, position_between};
use crate::filter::Filter;
use crate::forms::{
    BoardSwitcher, ColumnManager, Confirm, ConfirmAction, InputAction, InputModal, LabelPicker,
    Modal, TaskForm,
};
use crate::state::{BoardState, Tx, bootstrap, load_board_state};

/// Which screen is on top (modals layer above either one).
pub enum View {
    Board,
    Detail(DetailState),
}

pub struct DetailState {
    pub task_id: Id,
    pub sel_sub: usize,
    pub activities: Vec<Activity>,
    pub activities_loaded: bool,
}

#[derive(Debug)]
pub enum Message {
    Quit,
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    SelectTop,
    SelectBottom,
    OpenNewTask,
    OpenEditTask,
    OpenDeleteConfirm,
    ConfirmYes,
    CloseModal,
    FormInput(KeyEvent),
    FormNextField,
    FormPrevField,
    FormCyclePriority(i8),
    FormSubmit,
    MoveTaskLeft,
    MoveTaskRight,
    MoveTaskUp,
    MoveTaskDown,
    CyclePriority,
    OpenDetail,
    CloseDetail,
    DetailUp,
    DetailDown,
    ToggleSubtask,
    NewSubtask,
    EditSubtask,
    DeleteSubtask,
    OpenLabelPicker,
    LabelUp,
    LabelDown,
    ToggleLabel,
    NewLabelStart,
    NewLabelInput(KeyEvent),
    NewLabelSubmit,
    NewLabelCancel,
    DeleteLabel,
    OpenBoardSwitcher,
    BoardUp,
    BoardDown,
    SwitchBoard,
    NewBoardStart,
    RenameBoardStart,
    DeleteBoardStart,
    OpenColumnManager,
    ColUp,
    ColDown,
    ColMoveUp,
    ColMoveDown,
    NewColumnStart,
    RenameColumnStart,
    DeleteColumnStart,
    WipLimitStart,
    InputModalKey(KeyEvent),
    InputModalSubmit,
    FilterStart,
    FilterKey(KeyEvent),
    FilterConfirm,
    FilterClear,
    OpenGlobalSearch,
    GlobalSearchKey(KeyEvent),
    GlobalSearchDebounced(u64),
    GlobalSearchUp,
    GlobalSearchDown,
    GlobalSearchOpen,
    OpenHelp,
    ReloadBoard,
    ToastExpired(u64),
    Db(DbResult),
}

#[derive(Debug)]
pub enum DbResult {
    Bootstrapped(Vec<Board>, Box<BoardState>),
    BoardLoaded(Box<BoardState>),
    TaskCreated(Task),
    LabelCreated(crate::domain::Label),
    BoardCreated(Board),
    ColumnCreated(crate::domain::Column),
    ActivitiesLoaded(Id, Vec<Activity>),
    SearchResults(u64, Vec<Task>),
    Saved,
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Error,
}

pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub text: String,
}

pub struct App {
    pub db: Db,
    pub tx: Tx,
    pub should_quit: bool,
    pub boards: Vec<Board>,
    pub board: Option<BoardState>,
    pub sel_col: usize,
    pub sel_row: usize,
    pub view: View,
    pub modal: Option<Modal>,
    pub filter: Filter,
    pub search: Option<GlobalSearch>,
    pub toast: Option<Toast>,
    pub pending_saves: usize,
    pending_select: Option<Id>,
    dirty: bool,
    next_toast_id: u64,
}

pub struct GlobalSearch {
    pub input: String,
    pub results: Vec<Task>,
    pub sel: usize,
    pub seq: u64,
    pub searching: bool,
}

impl App {
    pub fn new(db: Db, tx: Tx) -> Self {
        Self {
            db,
            tx,
            should_quit: false,
            boards: Vec::new(),
            board: None,
            sel_col: 0,
            sel_row: 0,
            view: View::Board,
            modal: None,
            filter: Filter::default(),
            search: None,
            toast: None,
            pending_saves: 0,
            pending_select: None,
            dirty: true,
            next_toast_id: 0,
        }
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Spawn the initial load; result arrives as a `Bootstrapped` message.
    pub fn start(&self) {
        let db = self.db.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match bootstrap(&db).await {
                Ok((boards, state)) => DbResult::Bootstrapped(boards, Box::new(state)),
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::Quit => {
                self.should_quit = true;
            }
            Message::SelectLeft => self.move_selection(-1, 0),
            Message::SelectRight => self.move_selection(1, 0),
            Message::SelectUp => self.move_selection(0, -1),
            Message::SelectDown => self.move_selection(0, 1),
            Message::SelectTop => self.jump_selection(true),
            Message::SelectBottom => self.jump_selection(false),
            Message::OpenNewTask => self.open_new_task(),
            Message::OpenEditTask => self.open_edit_task(),
            Message::OpenDeleteConfirm => self.open_delete_confirm(),
            Message::ConfirmYes => self.confirm_yes(),
            Message::CloseModal => {
                let closed = self.modal.take().is_some() || self.search.take().is_some();
                if closed {
                    self.dirty = true;
                }
            }
            Message::FormInput(key) => self.form_input(key),
            Message::FormNextField => self.form_move_focus(true),
            Message::FormPrevField => self.form_move_focus(false),
            Message::FormCyclePriority(dir) => self.form_cycle_priority(dir),
            Message::FormSubmit => self.form_submit(),
            Message::MoveTaskLeft => self.move_task_horizontal(-1),
            Message::MoveTaskRight => self.move_task_horizontal(1),
            Message::MoveTaskUp => self.move_task_vertical(-1),
            Message::MoveTaskDown => self.move_task_vertical(1),
            Message::CyclePriority => self.cycle_selected_priority(),
            Message::OpenDetail => self.open_detail(),
            Message::CloseDetail => {
                if matches!(self.view, View::Detail(_)) {
                    self.view = View::Board;
                    self.dirty = true;
                }
            }
            Message::DetailUp => self.detail_select(-1),
            Message::DetailDown => self.detail_select(1),
            Message::ToggleSubtask => self.toggle_subtask(),
            Message::NewSubtask => self.open_new_subtask(),
            Message::EditSubtask => self.open_edit_subtask(),
            Message::DeleteSubtask => self.delete_selected_subtask(),
            Message::OpenLabelPicker => self.open_label_picker(),
            Message::LabelUp => self.label_select(-1),
            Message::LabelDown => self.label_select(1),
            Message::ToggleLabel => self.toggle_label(),
            Message::NewLabelStart => self.start_new_label(),
            Message::NewLabelInput(key) => self.new_label_input(key),
            Message::NewLabelSubmit => self.submit_new_label(),
            Message::NewLabelCancel => {
                if let Some(Modal::Labels(picker)) = &mut self.modal {
                    picker.adding = None;
                    self.dirty = true;
                }
            }
            Message::DeleteLabel => self.delete_selected_label(),
            Message::OpenBoardSwitcher => self.open_board_switcher(),
            Message::BoardUp => self.board_switcher_select(-1),
            Message::BoardDown => self.board_switcher_select(1),
            Message::SwitchBoard => self.switch_board(),
            Message::NewBoardStart => {
                self.open_input("New Board", InputAction::NewBoard, "", "Board name…")
            }
            Message::RenameBoardStart => self.rename_board_start(),
            Message::DeleteBoardStart => self.delete_board_start(),
            Message::OpenColumnManager => self.open_column_manager(),
            Message::ColUp => self.column_manager_select(-1),
            Message::ColDown => self.column_manager_select(1),
            Message::ColMoveUp => self.move_column(-1),
            Message::ColMoveDown => self.move_column(1),
            Message::NewColumnStart => {
                self.open_input("New Column", InputAction::NewColumn, "", "Column name…")
            }
            Message::RenameColumnStart => self.rename_column_start(),
            Message::DeleteColumnStart => self.delete_column_start(),
            Message::WipLimitStart => self.wip_limit_start(),
            Message::InputModalKey(key) => self.input_modal_key(key),
            Message::InputModalSubmit => self.input_modal_submit(),
            Message::FilterStart => {
                self.filter.editing = true;
                self.dirty = true;
            }
            Message::FilterKey(key) => self.filter_key(key),
            Message::FilterConfirm => {
                self.filter.editing = false;
                self.dirty = true;
            }
            Message::FilterClear => {
                self.filter.clear();
                self.clamp_selection();
                self.dirty = true;
            }
            Message::OpenGlobalSearch => {
                self.search = Some(GlobalSearch {
                    input: String::new(),
                    results: Vec::new(),
                    sel: 0,
                    seq: 0,
                    searching: false,
                });
                self.dirty = true;
            }
            Message::GlobalSearchKey(key) => self.global_search_key(key),
            Message::GlobalSearchDebounced(seq) => self.global_search_run(seq),
            Message::GlobalSearchUp => self.global_search_select(-1),
            Message::GlobalSearchDown => self.global_search_select(1),
            Message::GlobalSearchOpen => self.global_search_open(),
            Message::OpenHelp => {
                self.modal = Some(Modal::Help);
                self.dirty = true;
            }
            Message::ReloadBoard => self.reload_board(),
            Message::ToastExpired(id) => {
                if self.toast.as_ref().is_some_and(|t| t.id == id) {
                    self.toast = None;
                    self.dirty = true;
                }
            }
            Message::Db(result) => self.apply_db_result(result),
        }
    }

    fn apply_db_result(&mut self, result: DbResult) {
        self.dirty = true;
        match result {
            DbResult::Bootstrapped(boards, state) => {
                self.boards = boards;
                self.board = Some(*state);
                self.clamp_selection();
                self.validate_detail();
            }
            DbResult::BoardLoaded(state) => {
                let changed_board = self
                    .board
                    .as_ref()
                    .is_none_or(|b| b.board.id != state.board.id);
                self.board = Some(*state);
                if changed_board {
                    self.sel_col = 0;
                    self.sel_row = 0;
                    self.view = View::Board;
                }
                self.apply_pending_selection();
                self.clamp_selection();
                self.validate_detail();
            }
            DbResult::TaskCreated(task) => {
                let parent_id = task.parent_id;
                self.insert_created_task(task);
                self.pending_saves = self.pending_saves.saturating_sub(1);
                if let (Some(parent_id), View::Detail(detail)) = (parent_id, &self.view)
                    && detail.task_id == parent_id
                {
                    self.load_detail_activities(parent_id);
                }
            }
            DbResult::LabelCreated(label) => {
                self.pending_saves = self.pending_saves.saturating_sub(1);
                if let Some(board) = &mut self.board {
                    board.labels.push(label);
                    board
                        .labels
                        .sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
                }
            }
            DbResult::BoardCreated(board) => {
                self.pending_saves = self.pending_saves.saturating_sub(1);
                self.boards.push(board.clone());
                self.load_board(board);
                self.show_toast(ToastKind::Info, "Board created".into());
            }
            DbResult::ColumnCreated(column) => {
                self.pending_saves = self.pending_saves.saturating_sub(1);
                if let Some(board) = &mut self.board
                    && board.board.id == column.board_id
                {
                    board.columns.push(column);
                    board.tasks.push(Vec::new());
                }
            }
            DbResult::ActivitiesLoaded(task_id, activities) => {
                if let View::Detail(detail) = &mut self.view
                    && detail.task_id == task_id
                {
                    detail.activities = activities;
                    detail.activities_loaded = true;
                }
            }
            DbResult::SearchResults(seq, tasks) => {
                if let Some(search) = &mut self.search
                    && search.seq == seq
                {
                    search.results = tasks;
                    search.sel = search.sel.min(search.results.len().saturating_sub(1));
                    search.searching = false;
                }
            }
            DbResult::Saved => {
                self.pending_saves = self.pending_saves.saturating_sub(1);
            }
            DbResult::Error(text) => {
                self.pending_saves = self.pending_saves.saturating_sub(1);
                self.show_toast(ToastKind::Error, text);
                // In-memory state may have diverged from disk; reload to resync.
                self.reload_board();
            }
        }
    }

    /// Close the detail view if its task vanished (e.g. after an error resync).
    fn validate_detail(&mut self) {
        if let View::Detail(detail) = &self.view {
            let exists = self
                .board
                .as_ref()
                .is_some_and(|b| b.find_task(detail.task_id).is_some());
            if !exists {
                self.view = View::Board;
            } else {
                let len = self.detail_subtasks_len();
                if let View::Detail(detail) = &mut self.view {
                    detail.sel_sub = detail.sel_sub.min(len.saturating_sub(1));
                }
            }
        }
    }

    fn insert_created_task(&mut self, task: Task) {
        let Some(board) = &mut self.board else { return };
        match task.parent_id {
            Some(parent) => board.subtasks.entry(parent).or_default().push(task),
            None => {
                if let Some(idx) = board.column_index(task.column_id) {
                    board.tasks[idx].push(task);
                    if idx == self.sel_col {
                        self.sel_row = board.tasks[idx].len() - 1;
                    }
                }
            }
        }
    }

    fn move_selection(&mut self, dx: i64, dy: i64) {
        let Some(board) = &self.board else { return };
        if board.columns.is_empty() {
            return;
        }
        let cols = board.columns.len();
        let new_col = (self.sel_col as i64 + dx).clamp(0, cols as i64 - 1) as usize;
        let rows = self.visible_tasks(board, new_col).len();
        let new_row = if rows == 0 {
            0
        } else {
            (self.sel_row as i64 + dy).clamp(0, rows as i64 - 1) as usize
        };
        if new_col != self.sel_col || new_row != self.sel_row {
            self.sel_col = new_col;
            self.sel_row = new_row;
            self.dirty = true;
        }
    }

    fn jump_selection(&mut self, top: bool) {
        let Some(board) = &self.board else { return };
        if board.columns.is_empty() {
            return;
        }
        let rows = self.visible_tasks(board, self.sel_col).len();
        let new_row = if top || rows == 0 { 0 } else { rows - 1 };
        if new_row != self.sel_row {
            self.sel_row = new_row;
            self.dirty = true;
        }
    }

    /// Tasks of a column after the active filter; the selection row indexes
    /// into this list. Cheap: only builds a Vec of references.
    pub fn visible_tasks<'a>(&self, board: &'a BoardState, col: usize) -> Vec<&'a Task> {
        if self.filter.is_active() {
            board.tasks[col]
                .iter()
                .filter(|t| self.filter.matches(t, board))
                .collect()
        } else {
            board.tasks[col].iter().collect()
        }
    }

    pub fn filter_active(&self) -> bool {
        self.filter.is_active()
    }

    // ----- filter & global search -------------------------------------------

    fn filter_key(&mut self, key: KeyEvent) {
        match key.code {
            crossterm::event::KeyCode::Char(c) => self.filter.push(c),
            crossterm::event::KeyCode::Backspace => self.filter.pop(),
            _ => return,
        }
        self.clamp_selection();
        self.dirty = true;
    }

    fn global_search_key(&mut self, key: KeyEvent) {
        let Some(search) = &mut self.search else {
            return;
        };
        match key.code {
            crossterm::event::KeyCode::Char(c) => search.input.push(c),
            crossterm::event::KeyCode::Backspace => {
                search.input.pop();
            }
            _ => return,
        }
        search.seq += 1;
        search.sel = 0;
        let seq = search.seq;
        self.dirty = true;
        let tx = self.tx.clone();
        // Debounce: only the latest edit triggers a query.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let _ = tx.send(Message::GlobalSearchDebounced(seq));
        });
    }

    fn global_search_run(&mut self, seq: u64) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.seq != seq {
            return; // superseded by newer input
        }
        if search.input.trim().is_empty() {
            search.results.clear();
            search.searching = false;
            self.dirty = true;
            return;
        }
        search.searching = true;
        self.dirty = true;
        let query = search.input.clone();
        let db = self.db.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match db.search_tasks_global(&query, 50).await {
                Ok(tasks) => DbResult::SearchResults(seq, tasks),
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    fn global_search_select(&mut self, dir: i64) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.results.is_empty() {
            return;
        }
        let len = search.results.len() as i64;
        let new = (search.sel as i64 + dir).clamp(0, len - 1) as usize;
        if new != search.sel {
            search.sel = new;
            self.dirty = true;
        }
    }

    fn global_search_open(&mut self) {
        let Some(search) = self.search.take() else {
            return;
        };
        self.dirty = true;
        let Some(task) = search.results.get(search.sel) else {
            return;
        };
        self.pending_select = Some(task.id);
        if self
            .board
            .as_ref()
            .is_some_and(|b| b.board.id == task.board_id)
        {
            self.apply_pending_selection();
            return;
        }
        let Some(board) = self.boards.iter().find(|b| b.id == task.board_id).cloned() else {
            return;
        };
        self.load_board(board);
    }

    /// Move the cursor to a task we just navigated to (e.g. via global search).
    fn apply_pending_selection(&mut self) {
        let Some(task_id) = self.pending_select.take() else {
            return;
        };
        let Some(board) = &self.board else { return };
        let Some(task) = board.find_task(task_id) else {
            return;
        };
        let target_id = task.parent_id.unwrap_or(task.id);
        // A filter could hide the target; clear it so the jump always lands.
        self.filter.clear();
        for (col, tasks) in board.tasks.iter().enumerate() {
            if let Some(row) = tasks.iter().position(|t| t.id == target_id) {
                self.sel_col = col;
                self.sel_row = row;
                break;
            }
        }
    }

    /// The task currently under the cursor, if any.
    pub fn selected_task(&self) -> Option<&Task> {
        let board = self.board.as_ref()?;
        if board.columns.is_empty() {
            return None;
        }
        self.visible_tasks(board, self.sel_col)
            .get(self.sel_row)
            .copied()
    }

    // ----- modals ---------------------------------------------------------

    fn open_new_task(&mut self) {
        let Some(board) = &self.board else { return };
        let Some(column) = board.columns.get(self.sel_col) else {
            self.show_toast(ToastKind::Error, "Create a column first (c)".into());
            return;
        };
        self.modal = Some(Modal::TaskForm(Box::new(TaskForm::create_in(
            column.id, None,
        ))));
        self.dirty = true;
    }

    fn open_edit_task(&mut self) {
        let Some(task) = self.selected_task() else {
            return;
        };
        self.modal = Some(Modal::TaskForm(Box::new(TaskForm::edit(task))));
        self.dirty = true;
    }

    fn open_delete_confirm(&mut self) {
        let Some(task) = self.selected_task() else {
            return;
        };
        let n_subs = self
            .board
            .as_ref()
            .and_then(|b| b.subtasks.get(&task.id))
            .map_or(0, |s| s.len());
        let suffix = if n_subs > 0 {
            format!(" and its {n_subs} subtask(s)")
        } else {
            String::new()
        };
        self.modal = Some(Modal::Confirm(Confirm {
            text: format!("Delete {} \"{}\"{suffix}?", task.key, task.title),
            action: ConfirmAction::DeleteTask(task.id),
        }));
        self.dirty = true;
    }

    fn confirm_yes(&mut self) {
        let Some(Modal::Confirm(confirm)) = self.modal.take() else {
            return;
        };
        self.dirty = true;
        match confirm.action {
            ConfirmAction::DeleteTask(task_id) => self.delete_task(task_id),
            ConfirmAction::DeleteLabel(label_id) => self.delete_label(label_id),
            ConfirmAction::DeleteBoard(board_id) => self.delete_board(board_id),
            ConfirmAction::DeleteColumn(column_id) => self.delete_column(column_id),
        }
    }

    // ----- forms ----------------------------------------------------------

    fn form_input(&mut self, key: KeyEvent) {
        let Some(Modal::TaskForm(form)) = &mut self.modal else {
            return;
        };
        // Keep single-line fields single-line by rejecting Enter.
        let is_multiline = matches!(form.focus, crate::forms::FormFocus::Description);
        if let Some(textarea) = form.focused_textarea_mut() {
            let input = tui_textarea::Input::from(key);
            if !is_multiline && matches!(input.key, tui_textarea::Key::Enter) {
                return;
            }
            if textarea.input(input) {
                form.error = None;
            }
            self.dirty = true;
        }
    }

    fn form_move_focus(&mut self, forward: bool) {
        let Some(Modal::TaskForm(form)) = &mut self.modal else {
            return;
        };
        form.focus = if forward {
            form.focus.next()
        } else {
            form.focus.prev()
        };
        self.dirty = true;
    }

    fn form_cycle_priority(&mut self, dir: i8) {
        let Some(Modal::TaskForm(form)) = &mut self.modal else {
            return;
        };
        form.priority = if dir >= 0 {
            form.priority.cycle()
        } else {
            // three forward steps == one back in a 4-cycle
            form.priority.cycle().cycle().cycle()
        };
        self.dirty = true;
    }

    fn form_submit(&mut self) {
        let Some(Modal::TaskForm(form)) = &mut self.modal else {
            return;
        };
        let output = match form.validate() {
            Ok(output) => output,
            Err(error) => {
                form.error = Some(error);
                self.dirty = true;
                return;
            }
        };
        let editing = form.editing;
        let column_id = form.column_id;
        let parent_id = form.parent_id;
        self.modal = None;
        self.dirty = true;

        match editing {
            None => {
                let Some(board) = &self.board else { return };
                let new = NewTask {
                    board_id: board.board.id,
                    column_id,
                    parent_id,
                    title: output.title,
                    description: output.description,
                    priority: output.priority,
                    due_date: output.due_date,
                };
                let db = self.db.clone();
                let tx = self.tx.clone();
                self.pending_saves += 1;
                tokio::spawn(async move {
                    let msg = match db.create_task(new).await {
                        Ok(task) => DbResult::TaskCreated(task),
                        Err(err) => DbResult::Error(err.to_string()),
                    };
                    let _ = tx.send(Message::Db(msg));
                });
            }
            Some(task_id) => self.apply_task_edit(task_id, output),
        }
    }

    fn apply_task_edit(&mut self, task_id: Id, output: crate::forms::FormOutput) {
        let Some(board) = &mut self.board else { return };
        let Some(task) = board.find_task_mut(task_id) else {
            return;
        };
        let mut changes = Vec::new();
        if task.title != output.title {
            changes.push("title");
        }
        if task.description != output.description {
            changes.push("description");
        }
        if task.priority != output.priority {
            changes.push("priority");
        }
        if task.due_date != output.due_date {
            changes.push("due date");
        }
        task.title = output.title.clone();
        task.description = output.description.clone();
        task.priority = output.priority;
        task.due_date = output.due_date;

        if changes.is_empty() {
            return;
        }
        let detail = format!("Updated {}", changes.join(", "));
        let db = self.db.clone();
        self.persist(async move {
            db.update_task_content(
                task_id,
                &output.title,
                &output.description,
                output.priority,
                output.due_date,
            )
            .await?;
            db.log_activity(task_id, activity_kind::EDITED, &detail)
                .await
        });
    }

    // ----- detail view ------------------------------------------------------

    fn open_detail(&mut self) {
        let Some(task) = self.selected_task() else {
            return;
        };
        let task_id = task.id;
        self.view = View::Detail(DetailState {
            task_id,
            sel_sub: 0,
            activities: Vec::new(),
            activities_loaded: false,
        });
        self.dirty = true;
        self.load_detail_activities(task_id);
    }

    fn load_detail_activities(&self, task_id: Id) {
        let db = self.db.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match db.activities_for_task(task_id).await {
                Ok(acts) => DbResult::ActivitiesLoaded(task_id, acts),
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    fn detail_subtasks_len(&self) -> usize {
        let View::Detail(detail) = &self.view else {
            return 0;
        };
        self.board
            .as_ref()
            .and_then(|b| b.subtasks.get(&detail.task_id))
            .map_or(0, |s| s.len())
    }

    fn detail_select(&mut self, dir: i64) {
        let len = self.detail_subtasks_len();
        let View::Detail(detail) = &mut self.view else {
            return;
        };
        if len == 0 {
            return;
        }
        let new = (detail.sel_sub as i64 + dir).clamp(0, len as i64 - 1) as usize;
        if new != detail.sel_sub {
            detail.sel_sub = new;
            self.dirty = true;
        }
    }

    fn selected_subtask(&self) -> Option<&Task> {
        let View::Detail(detail) = &self.view else {
            return None;
        };
        self.board
            .as_ref()?
            .subtasks
            .get(&detail.task_id)?
            .get(detail.sel_sub)
    }

    fn toggle_subtask(&mut self) {
        let Some(sub) = self.selected_subtask() else {
            return;
        };
        let (sub_id, parent_id) = (sub.id, sub.parent_id);
        let Some(board) = &mut self.board else { return };
        let Some(sub) = board.find_task_mut(sub_id) else {
            return;
        };
        sub.done = !sub.done;
        let done = sub.done;
        let title = sub.title.clone();
        self.dirty = true;

        if let Some(parent_id) = parent_id {
            let db = self.db.clone();
            let detail = format!(
                "{} subtask \"{}\"",
                if done { "Completed" } else { "Reopened" },
                title
            );
            self.persist_and_refresh_activities(parent_id, async move {
                db.set_task_done(sub_id, done).await?;
                db.log_activity(parent_id, activity_kind::SUBTASK, &detail)
                    .await
            });
        }
    }

    fn open_new_subtask(&mut self) {
        let View::Detail(detail) = &self.view else {
            return;
        };
        let task_id = detail.task_id;
        let Some(board) = &self.board else { return };
        let Some(parent) = board.find_task(task_id) else {
            return;
        };
        self.modal = Some(Modal::TaskForm(Box::new(TaskForm::create_in(
            parent.column_id,
            Some(task_id),
        ))));
        self.dirty = true;
    }

    fn open_edit_subtask(&mut self) {
        let Some(sub) = self.selected_subtask() else {
            return;
        };
        self.modal = Some(Modal::TaskForm(Box::new(TaskForm::edit(sub))));
        self.dirty = true;
    }

    fn delete_selected_subtask(&mut self) {
        let Some(sub) = self.selected_subtask() else {
            return;
        };
        self.modal = Some(Modal::Confirm(Confirm {
            text: format!("Delete subtask \"{}\"?", sub.title),
            action: ConfirmAction::DeleteTask(sub.id),
        }));
        self.dirty = true;
    }

    /// Like `persist`, but re-fetches the activity log of `task_id` afterwards
    /// so an open detail view stays current.
    fn persist_and_refresh_activities<F>(&mut self, task_id: Id, fut: F)
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.pending_saves += 1;
        self.dirty = true;
        let db = self.db.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match fut.await {
                Ok(()) => {
                    if let Ok(acts) = db.activities_for_task(task_id).await {
                        let _ = tx.send(Message::Db(DbResult::ActivitiesLoaded(task_id, acts)));
                    }
                    DbResult::Saved
                }
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    // ----- labels -----------------------------------------------------------

    fn open_label_picker(&mut self) {
        let task_id = match &self.view {
            View::Detail(detail) => Some(detail.task_id),
            View::Board => self.selected_task().map(|t| t.id),
        };
        let Some(task_id) = task_id else { return };
        self.modal = Some(Modal::Labels(LabelPicker::new(task_id)));
        self.dirty = true;
    }

    fn label_select(&mut self, dir: i64) {
        let len = self.board.as_ref().map_or(0, |b| b.labels.len());
        let Some(Modal::Labels(picker)) = &mut self.modal else {
            return;
        };
        if len == 0 {
            return;
        }
        let new = (picker.sel as i64 + dir).clamp(0, len as i64 - 1) as usize;
        if new != picker.sel {
            picker.sel = new;
            self.dirty = true;
        }
    }

    fn toggle_label(&mut self) {
        let Some(Modal::Labels(picker)) = &self.modal else {
            return;
        };
        let (task_id, sel) = (picker.task_id, picker.sel);
        let Some(board) = &mut self.board else { return };
        let Some(label) = board.labels.get(sel) else {
            return;
        };
        let (label_id, label_name) = (label.id, label.name.clone());

        let ids = board.task_labels.entry(task_id).or_default();
        let added = if let Some(pos) = ids.iter().position(|&id| id == label_id) {
            ids.remove(pos);
            false
        } else {
            ids.push(label_id);
            true
        };
        let ids = ids.clone();
        if board.task_labels.get(&task_id).is_some_and(Vec::is_empty) {
            board.task_labels.remove(&task_id);
        }
        self.dirty = true;

        let db = self.db.clone();
        let detail = format!(
            "{} label \"{label_name}\"",
            if added { "Added" } else { "Removed" }
        );
        self.persist_and_refresh_activities(task_id, async move {
            db.set_task_labels(task_id, &ids).await?;
            db.log_activity(task_id, activity_kind::LABELS, &detail)
                .await
        });
    }

    fn start_new_label(&mut self) {
        let Some(Modal::Labels(picker)) = &mut self.modal else {
            return;
        };
        picker.start_adding();
        self.dirty = true;
    }

    fn new_label_input(&mut self, key: KeyEvent) {
        let Some(Modal::Labels(picker)) = &mut self.modal else {
            return;
        };
        let Some(textarea) = &mut picker.adding else {
            return;
        };
        let input = tui_textarea::Input::from(key);
        if !matches!(input.key, tui_textarea::Key::Enter) {
            textarea.input(input);
            self.dirty = true;
        }
    }

    fn submit_new_label(&mut self) {
        let Some(Modal::Labels(picker)) = &mut self.modal else {
            return;
        };
        let Some(textarea) = picker.adding.take() else {
            return;
        };
        self.dirty = true;
        let name = textarea.lines().join(" ").trim().to_string();
        if name.is_empty() {
            return;
        }
        let Some(board) = &self.board else { return };
        if board.labels.iter().any(|l| l.name == name) {
            self.show_toast(ToastKind::Error, format!("Label \"{name}\" already exists"));
            return;
        }
        let board_id = board.board.id;
        let color = board.labels.len() as i64 % crate::ui::theme::LABEL_PALETTE.len() as i64;
        let db = self.db.clone();
        let tx = self.tx.clone();
        self.pending_saves += 1;
        tokio::spawn(async move {
            let msg = match db.create_label(board_id, &name, color).await {
                Ok(label) => DbResult::LabelCreated(label),
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    fn delete_selected_label(&mut self) {
        let Some(Modal::Labels(picker)) = &self.modal else {
            return;
        };
        let sel = picker.sel;
        let Some(board) = &self.board else { return };
        let Some(label) = board.labels.get(sel) else {
            return;
        };
        self.modal = Some(Modal::Confirm(Confirm {
            text: format!(
                "Delete label \"{}\" from this board? It will be removed from every task.",
                label.name
            ),
            action: ConfirmAction::DeleteLabel(label.id),
        }));
        self.dirty = true;
    }

    fn delete_label(&mut self, label_id: Id) {
        let Some(board) = &mut self.board else { return };
        board.labels.retain(|l| l.id != label_id);
        for ids in board.task_labels.values_mut() {
            ids.retain(|&id| id != label_id);
        }
        board.task_labels.retain(|_, ids| !ids.is_empty());
        let db = self.db.clone();
        self.persist(async move { db.delete_label(label_id).await });
    }

    // ----- boards & columns management --------------------------------------

    fn open_board_switcher(&mut self) {
        let current = self
            .board
            .as_ref()
            .and_then(|b| self.boards.iter().position(|x| x.id == b.board.id))
            .unwrap_or(0);
        self.modal = Some(Modal::Boards(BoardSwitcher { sel: current }));
        self.dirty = true;
    }

    fn board_switcher_select(&mut self, dir: i64) {
        let len = self.boards.len();
        let Some(Modal::Boards(switcher)) = &mut self.modal else {
            return;
        };
        if len == 0 {
            return;
        }
        let new = (switcher.sel as i64 + dir).clamp(0, len as i64 - 1) as usize;
        if new != switcher.sel {
            switcher.sel = new;
            self.dirty = true;
        }
    }

    fn switch_board(&mut self) {
        let Some(Modal::Boards(switcher)) = &self.modal else {
            return;
        };
        let Some(board) = self.boards.get(switcher.sel).cloned() else {
            return;
        };
        self.modal = None;
        self.dirty = true;
        if self.board.as_ref().is_some_and(|b| b.board.id == board.id) {
            return;
        }
        self.load_board(board);
    }

    fn load_board(&mut self, board: Board) {
        let db = self.db.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match load_board_state(&db, board).await {
                Ok(state) => DbResult::BoardLoaded(Box::new(state)),
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    fn rename_board_start(&mut self) {
        let Some(Modal::Boards(switcher)) = &self.modal else {
            return;
        };
        let Some(board) = self.boards.get(switcher.sel) else {
            return;
        };
        let (id, name) = (board.id, board.name.clone());
        self.open_input(
            "Rename Board",
            InputAction::RenameBoard(id),
            &name,
            "Board name…",
        );
    }

    fn delete_board_start(&mut self) {
        let Some(Modal::Boards(switcher)) = &self.modal else {
            return;
        };
        if self.boards.len() <= 1 {
            self.show_toast(ToastKind::Error, "Cannot delete the last board".into());
            return;
        }
        let Some(board) = self.boards.get(switcher.sel) else {
            return;
        };
        self.modal = Some(Modal::Confirm(Confirm {
            text: format!(
                "Delete board \"{}\" and ALL of its tasks? This cannot be undone.",
                board.name
            ),
            action: ConfirmAction::DeleteBoard(board.id),
        }));
        self.dirty = true;
    }

    fn delete_board(&mut self, board_id: Id) {
        self.boards.retain(|b| b.id != board_id);
        if self.board.as_ref().is_some_and(|b| b.board.id == board_id)
            && let Some(first) = self.boards.first().cloned()
        {
            self.load_board(first);
        }
        let db = self.db.clone();
        self.persist(async move { db.delete_board(board_id).await });
    }

    fn open_column_manager(&mut self) {
        if self.board.is_none() {
            return;
        }
        self.modal = Some(Modal::Columns(ColumnManager { sel: self.sel_col }));
        self.dirty = true;
    }

    fn column_manager_select(&mut self, dir: i64) {
        let len = self.board.as_ref().map_or(0, |b| b.columns.len());
        let Some(Modal::Columns(manager)) = &mut self.modal else {
            return;
        };
        if len == 0 {
            return;
        }
        let new = (manager.sel as i64 + dir).clamp(0, len as i64 - 1) as usize;
        if new != manager.sel {
            manager.sel = new;
            self.dirty = true;
        }
    }

    fn move_column(&mut self, dir: i64) {
        let Some(Modal::Columns(manager)) = &self.modal else {
            return;
        };
        let sel = manager.sel;
        let Some(board) = &mut self.board else { return };
        let other = sel as i64 + dir;
        if other < 0 || other as usize >= board.columns.len() {
            return;
        }
        let other = other as usize;

        let (a_pos, b_pos) = (board.columns[sel].position, board.columns[other].position);
        let (a_id, b_id) = (board.columns[sel].id, board.columns[other].id);
        board.columns[sel].position = b_pos;
        board.columns[other].position = a_pos;
        board.columns.swap(sel, other);
        board.tasks.swap(sel, other);
        if let Some(Modal::Columns(manager)) = &mut self.modal {
            manager.sel = other;
        }
        self.dirty = true;

        let db = self.db.clone();
        self.persist(async move { db.swap_column_positions(a_id, a_pos, b_id, b_pos).await });
    }

    fn rename_column_start(&mut self) {
        let Some(Modal::Columns(manager)) = &self.modal else {
            return;
        };
        let Some(board) = &self.board else { return };
        let Some(column) = board.columns.get(manager.sel) else {
            return;
        };
        let (id, name) = (column.id, column.name.clone());
        self.open_input(
            "Rename Column",
            InputAction::RenameColumn(id),
            &name,
            "Column name…",
        );
    }

    fn delete_column_start(&mut self) {
        let Some(Modal::Columns(manager)) = &self.modal else {
            return;
        };
        let sel = manager.sel;
        let Some(board) = &self.board else { return };
        let Some(column) = board.columns.get(sel) else {
            return;
        };
        if !board.tasks[sel].is_empty() {
            self.show_toast(
                ToastKind::Error,
                "Column is not empty — move its tasks out first".into(),
            );
            return;
        }
        self.modal = Some(Modal::Confirm(Confirm {
            text: format!("Delete empty column \"{}\"?", column.name),
            action: ConfirmAction::DeleteColumn(column.id),
        }));
        self.dirty = true;
    }

    fn delete_column(&mut self, column_id: Id) {
        let Some(board) = &mut self.board else { return };
        if let Some(idx) = board.column_index(column_id) {
            board.columns.remove(idx);
            board.tasks.remove(idx);
        }
        self.clamp_selection();
        let db = self.db.clone();
        self.persist(async move { db.delete_column(column_id).await });
    }

    fn wip_limit_start(&mut self) {
        let Some(Modal::Columns(manager)) = &self.modal else {
            return;
        };
        let Some(board) = &self.board else { return };
        let Some(column) = board.columns.get(manager.sel) else {
            return;
        };
        let initial = column.wip_limit.map(|w| w.to_string()).unwrap_or_default();
        let id = column.id;
        self.open_input(
            "WIP Limit",
            InputAction::WipLimit(id),
            &initial,
            "Max cards (empty = no limit)",
        );
    }

    fn open_input(&mut self, title: &str, action: InputAction, initial: &str, placeholder: &str) {
        self.modal = Some(Modal::Input(InputModal::new(
            title,
            action,
            initial,
            placeholder,
        )));
        self.dirty = true;
    }

    fn input_modal_key(&mut self, key: KeyEvent) {
        let Some(Modal::Input(modal)) = &mut self.modal else {
            return;
        };
        let input = tui_textarea::Input::from(key);
        if !matches!(input.key, tui_textarea::Key::Enter) && modal.textarea.input(input) {
            modal.error = None;
        }
        self.dirty = true;
    }

    fn input_modal_submit(&mut self) {
        let Some(Modal::Input(modal)) = &mut self.modal else {
            return;
        };
        let value = modal.value();
        let action = modal.action;
        match action {
            InputAction::NewColumn
            | InputAction::NewBoard
            | InputAction::RenameColumn(_)
            | InputAction::RenameBoard(_)
                if value.is_empty() =>
            {
                modal.error = Some("Name cannot be empty".into());
                self.dirty = true;
                return;
            }
            InputAction::WipLimit(_) if !value.is_empty() && value.parse::<u32>().is_err() => {
                modal.error = Some("Enter a positive number or leave empty".into());
                self.dirty = true;
                return;
            }
            _ => {}
        }
        self.modal = None;
        self.dirty = true;

        match action {
            InputAction::NewColumn => {
                let Some(board) = &self.board else { return };
                let board_id = board.board.id;
                let db = self.db.clone();
                let tx = self.tx.clone();
                self.pending_saves += 1;
                tokio::spawn(async move {
                    let msg = match db.create_column(board_id, &value).await {
                        Ok(column) => DbResult::ColumnCreated(column),
                        Err(err) => DbResult::Error(err.to_string()),
                    };
                    let _ = tx.send(Message::Db(msg));
                });
            }
            InputAction::RenameColumn(id) => {
                if let Some(board) = &mut self.board
                    && let Some(column) = board.columns.iter_mut().find(|c| c.id == id)
                {
                    column.name = value.clone();
                }
                let db = self.db.clone();
                self.persist(async move { db.rename_column(id, &value).await });
            }
            InputAction::NewBoard => {
                let existing: Vec<String> = self.boards.iter().map(|b| b.key.clone()).collect();
                let key = crate::domain::derive_board_key(&value, &existing);
                let db = self.db.clone();
                let tx = self.tx.clone();
                self.pending_saves += 1;
                tokio::spawn(async move {
                    let msg = match db.create_board(&value, &key).await {
                        Ok(board) => DbResult::BoardCreated(board),
                        Err(err) => DbResult::Error(err.to_string()),
                    };
                    let _ = tx.send(Message::Db(msg));
                });
            }
            InputAction::RenameBoard(id) => {
                if let Some(board) = self.boards.iter_mut().find(|b| b.id == id) {
                    board.name = value.clone();
                }
                if let Some(state) = &mut self.board
                    && state.board.id == id
                {
                    state.board.name = value.clone();
                }
                let db = self.db.clone();
                self.persist(async move { db.rename_board(id, &value).await });
            }
            InputAction::WipLimit(id) => {
                let limit: Option<i64> = if value.is_empty() {
                    None
                } else {
                    value.parse::<i64>().ok()
                };
                if let Some(board) = &mut self.board
                    && let Some(column) = board.columns.iter_mut().find(|c| c.id == id)
                {
                    column.wip_limit = limit;
                }
                let db = self.db.clone();
                self.persist(async move { db.set_wip_limit(id, limit).await });
            }
        }
    }

    // ----- task operations (optimistic) -------------------------------------

    fn delete_task(&mut self, task_id: Id) {
        let Some(board) = &mut self.board else { return };
        let mut removed_subtask: Option<(Id, String)> = None;
        let mut found = false;
        for tasks in &mut board.tasks {
            if let Some(idx) = tasks.iter().position(|t| t.id == task_id) {
                tasks.remove(idx);
                found = true;
                break;
            }
        }
        if !found {
            // Not a card — it's a subtask: remove from its parent's list.
            for (parent_id, subs) in board.subtasks.iter_mut() {
                if let Some(idx) = subs.iter().position(|t| t.id == task_id) {
                    let sub = subs.remove(idx);
                    removed_subtask = Some((*parent_id, sub.title));
                    break;
                }
            }
        }
        board.subtasks.remove(&task_id);
        self.clamp_selection();
        self.validate_detail();

        let db = self.db.clone();
        match removed_subtask {
            Some((parent_id, title)) => {
                let detail = format!("Removed subtask \"{title}\"");
                self.persist_and_refresh_activities(parent_id, async move {
                    db.delete_task(task_id).await?;
                    db.log_activity(parent_id, activity_kind::SUBTASK, &detail)
                        .await
                });
            }
            None => self.persist(async move { db.delete_task(task_id).await }),
        }
    }

    fn move_task_horizontal(&mut self, dir: i64) {
        let Some(task) = self.selected_task() else {
            return;
        };
        let (task_id, src_column_id) = (task.id, task.column_id);
        let Some(board) = &mut self.board else { return };
        let Some(src_col) = board.column_index(src_column_id) else {
            return;
        };
        let dst_col = src_col as i64 + dir;
        if dst_col < 0 || dst_col as usize >= board.columns.len() {
            return;
        }
        let dst_col = dst_col as usize;

        let Some(src_idx) = board.tasks[src_col].iter().position(|t| t.id == task_id) else {
            return;
        };
        let mut task = board.tasks[src_col].remove(src_idx);
        let new_pos = position_between(board.tasks[dst_col].last().map(|t| t.position), None)
            .unwrap_or(crate::domain::POSITION_GAP);
        let dst_column_id = board.columns[dst_col].id;
        let detail = format!(
            "Moved from {} to {}",
            board.columns[src_col].name, board.columns[dst_col].name
        );
        task.column_id = dst_column_id;
        task.position = new_pos;
        board.tasks[dst_col].push(task);

        // Selection follows the card.
        self.sel_col = dst_col;
        self.sel_row = self
            .board
            .as_ref()
            .map_or(0, |b| b.tasks[dst_col].len() - 1);
        self.dirty = true;

        let db = self.db.clone();
        self.persist(async move {
            db.move_task(task_id, dst_column_id, new_pos).await?;
            db.log_activity(task_id, activity_kind::MOVED, &detail)
                .await
        });
    }

    fn move_task_vertical(&mut self, dir: i64) {
        let Some(task) = self.selected_task() else {
            return;
        };
        let (task_id, column_id) = (task.id, task.column_id);
        let Some(board) = &mut self.board else { return };
        let Some(col) = board.column_index(column_id) else {
            return;
        };
        let tasks = &mut board.tasks[col];
        let Some(idx) = tasks.iter().position(|t| t.id == task_id) else {
            return;
        };
        let other = idx as i64 + dir;
        if other < 0 || other as usize >= tasks.len() {
            return;
        }
        let other = other as usize;

        // Swap stored positions, then swap in the vec — order stays consistent
        // and no renumbering is ever required.
        let (a_pos, b_pos) = (tasks[idx].position, tasks[other].position);
        tasks[idx].position = b_pos;
        tasks[other].position = a_pos;
        let other_id = tasks[other].id;
        tasks.swap(idx, other);
        self.sel_row = other;
        self.dirty = true;

        let db = self.db.clone();
        self.persist(async move {
            db.swap_task_positions(task_id, a_pos, other_id, b_pos)
                .await
        });
    }

    fn cycle_selected_priority(&mut self) {
        let Some(task) = self.selected_task() else {
            return;
        };
        let task_id = task.id;
        let Some(board) = &mut self.board else { return };
        let Some(task) = board.find_task_mut(task_id) else {
            return;
        };
        task.priority = task.priority.cycle();
        let new_priority = task.priority;
        self.dirty = true;

        let db = self.db.clone();
        self.persist(async move {
            db.set_task_priority(task_id, new_priority).await?;
            db.log_activity(
                task_id,
                activity_kind::PRIORITY,
                &format!("Priority set to {}", new_priority.name()),
            )
            .await
        });
    }

    fn clamp_selection(&mut self) {
        let Some(board) = &self.board else { return };
        if board.columns.is_empty() {
            self.sel_col = 0;
            self.sel_row = 0;
            return;
        }
        self.sel_col = self.sel_col.min(board.columns.len() - 1);
        let rows = self.visible_tasks(board, self.sel_col).len();
        self.sel_row = if rows == 0 {
            0
        } else {
            self.sel_row.min(rows - 1)
        };
    }

    pub fn reload_board(&mut self) {
        let Some(board) = &self.board else { return };
        let board_meta = board.board.clone();
        let db = self.db.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match load_board_state(&db, board_meta).await {
                Ok(state) => DbResult::BoardLoaded(Box::new(state)),
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    /// Fire-and-forget persistence of an optimistic update. Tracks the write in
    /// the "saving" counter and reports failures as toasts (plus a resync).
    pub fn persist<F>(&mut self, fut: F)
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.pending_saves += 1;
        self.dirty = true;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let msg = match fut.await {
                Ok(()) => DbResult::Saved,
                Err(err) => DbResult::Error(err.to_string()),
            };
            let _ = tx.send(Message::Db(msg));
        });
    }

    pub fn show_toast(&mut self, kind: ToastKind, text: String) {
        self.next_toast_id += 1;
        let id = self.next_toast_id;
        self.toast = Some(Toast { id, kind, text });
        self.dirty = true;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(3500)).await;
            let _ = tx.send(Message::ToastExpired(id));
        });
    }
}

pub fn channel() -> (Tx, mpsc::UnboundedReceiver<Message>) {
    mpsc::unbounded_channel()
}
