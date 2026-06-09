CREATE TABLE boards (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT    NOT NULL,
    key           TEXT    NOT NULL UNIQUE,
    next_task_num INTEGER NOT NULL DEFAULT 1,
    position      INTEGER NOT NULL,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE columns (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id  INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name      TEXT    NOT NULL,
    position  INTEGER NOT NULL,
    wip_limit INTEGER
);

CREATE INDEX idx_columns_board ON columns(board_id, position);

CREATE TABLE tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id    INTEGER NOT NULL REFERENCES boards(id)  ON DELETE CASCADE,
    column_id   INTEGER NOT NULL REFERENCES columns(id) ON DELETE CASCADE,
    parent_id   INTEGER          REFERENCES tasks(id)   ON DELETE CASCADE,
    key         TEXT    NOT NULL,
    title       TEXT    NOT NULL,
    description TEXT    NOT NULL DEFAULT '',
    priority    INTEGER NOT NULL DEFAULT 1,
    position    INTEGER NOT NULL,
    due_date    TEXT,
    done        INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL
);

CREATE INDEX idx_tasks_board_col_pos ON tasks(board_id, column_id, position);
CREATE INDEX idx_tasks_parent ON tasks(parent_id);

CREATE TABLE labels (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name     TEXT    NOT NULL,
    color    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_labels_board ON labels(board_id);

CREATE TABLE task_labels (
    task_id  INTEGER NOT NULL REFERENCES tasks(id)  ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, label_id)
) WITHOUT ROWID;

CREATE TABLE activities (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id    INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    kind       TEXT    NOT NULL,
    detail     TEXT    NOT NULL DEFAULT '',
    created_at TEXT    NOT NULL
);

CREATE INDEX idx_activities_task ON activities(task_id, created_at);
