use super::Db;
use crate::db::tasks::NewTask;
use crate::domain::{ActivityKind, Priority, position_between};

async fn db() -> Db {
    Db::connect_in_memory().await.expect("in-memory db")
}

fn new_task(board_id: i64, column_id: i64, title: &str) -> NewTask {
    NewTask {
        board_id,
        column_id,
        parent_id: None,
        title: title.to_string(),
        description: String::new(),
        priority: Priority::Medium,
        due_date: None,
    }
}

#[tokio::test]
async fn create_board_creates_default_columns() {
    let db = db().await;
    let board = db.create_board("My Project", "MP").await.unwrap();
    assert_eq!(board.key, "MP");
    let columns = db.columns_for_board(board.id).await.unwrap();
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["To Do", "In Progress", "Done"]);
}

#[tokio::test]
async fn task_keys_increment_per_board() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    let t1 = db
        .create_task(new_task(board.id, columns[0].id, "first"))
        .await
        .unwrap();
    let t2 = db
        .create_task(new_task(board.id, columns[0].id, "second"))
        .await
        .unwrap();
    assert_eq!(t1.key, "TSK-1");
    assert_eq!(t2.key, "TSK-2");
    assert!(t1.position < t2.position);

    let other = db.create_board("Other", "OTH").await.unwrap();
    let other_cols = db.columns_for_board(other.id).await.unwrap();
    let t3 = db
        .create_task(new_task(other.id, other_cols[0].id, "elsewhere"))
        .await
        .unwrap();
    assert_eq!(t3.key, "OTH-1");
}

#[tokio::test]
async fn move_task_with_sparse_positions() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    let a = db
        .create_task(new_task(board.id, columns[0].id, "a"))
        .await
        .unwrap();
    let b = db
        .create_task(new_task(board.id, columns[0].id, "b"))
        .await
        .unwrap();

    // Move b before a using a midpoint position — single UPDATE.
    let pos = position_between(None, Some(a.position)).unwrap();
    db.move_task(b.id, columns[0].id, pos).await.unwrap();
    let tasks = db.tasks_for_board(board.id).await.unwrap();
    assert_eq!(tasks[0].title, "b");
    assert_eq!(tasks[1].title, "a");

    // Move a to another column.
    db.move_task(a.id, columns[1].id, 1024).await.unwrap();
    let tasks = db.tasks_for_board(board.id).await.unwrap();
    let a_now = tasks.iter().find(|t| t.id == a.id).unwrap();
    assert_eq!(a_now.column_id, columns[1].id);
}

#[tokio::test]
async fn renumber_tasks_restores_gaps() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    for i in 0..5 {
        db.create_task(new_task(board.id, columns[0].id, &format!("t{i}")))
            .await
            .unwrap();
    }
    let renumbered = db.renumber_tasks(columns[0].id).await.unwrap();
    let positions: Vec<i64> = renumbered.iter().map(|t| t.position).collect();
    assert_eq!(positions, [1024, 2048, 3072, 4096, 5120]);
}

#[tokio::test]
async fn subtasks_scope_positions_to_parent() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    let parent = db
        .create_task(new_task(board.id, columns[0].id, "parent"))
        .await
        .unwrap();
    let mut sub = new_task(board.id, columns[0].id, "child");
    sub.parent_id = Some(parent.id);
    let child = db.create_task(sub).await.unwrap();
    assert_eq!(child.parent_id, Some(parent.id));
    assert_eq!(child.key, "TSK-2");

    db.set_task_done(child.id, true).await.unwrap();
    let tasks = db.tasks_for_board(board.id).await.unwrap();
    assert!(tasks.iter().find(|t| t.id == child.id).unwrap().done);

    // Deleting the parent cascades to the child.
    db.delete_task(parent.id).await.unwrap();
    assert!(db.tasks_for_board(board.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn labels_round_trip_and_cascade() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    let task = db
        .create_task(new_task(board.id, columns[0].id, "t"))
        .await
        .unwrap();
    let bug = db.create_label(board.id, "bug", 0).await.unwrap();
    let feat = db.create_label(board.id, "feature", 3).await.unwrap();

    db.set_task_labels(task.id, &[bug.id, feat.id])
        .await
        .unwrap();
    let pairs = db.task_label_pairs(board.id).await.unwrap();
    assert_eq!(pairs.len(), 2);

    db.delete_label(bug.id).await.unwrap();
    let pairs = db.task_label_pairs(board.id).await.unwrap();
    assert_eq!(pairs, vec![(task.id, feat.id)]);
}

#[tokio::test]
async fn activities_are_logged_and_listed() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    let task = db
        .create_task(new_task(board.id, columns[0].id, "t"))
        .await
        .unwrap();
    db.log_activity(task.id, ActivityKind::Moved, "moved from To Do to Done")
        .await
        .unwrap();
    let acts = db.activities_for_task(task.id).await.unwrap();
    assert_eq!(acts.len(), 2); // "created" + "moved"
    assert_eq!(acts[0].kind, ActivityKind::Moved);
}

#[tokio::test]
async fn delete_board_cascades_everything() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    let task = db
        .create_task(new_task(board.id, columns[0].id, "t"))
        .await
        .unwrap();
    let label = db.create_label(board.id, "bug", 0).await.unwrap();
    db.set_task_labels(task.id, &[label.id]).await.unwrap();

    db.delete_board(board.id).await.unwrap();
    assert!(db.list_boards().await.unwrap().is_empty());
    assert!(db.columns_for_board(board.id).await.unwrap().is_empty());
    assert!(db.tasks_for_board(board.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn global_search_matches_key_and_title() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    db.create_task(new_task(board.id, columns[0].id, "Fix login bug"))
        .await
        .unwrap();
    db.create_task(new_task(board.id, columns[0].id, "Write docs"))
        .await
        .unwrap();

    let hits = db.search_tasks_global("LOGIN", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Fix login bug");

    let hits = db.search_tasks_global("tsk-2", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Write docs");
}

#[tokio::test]
async fn global_search_escapes_like_metacharacters() {
    let db = db().await;
    let board = db.create_board("Tasko", "TSK").await.unwrap();
    let columns = db.columns_for_board(board.id).await.unwrap();
    db.create_task(new_task(board.id, columns[0].id, "path C:\\temp report"))
        .await
        .unwrap();
    db.create_task(new_task(board.id, columns[0].id, "progress 50% done"))
        .await
        .unwrap();
    db.create_task(new_task(board.id, columns[0].id, "snake_case name"))
        .await
        .unwrap();

    // A backslash in the query must match literally (was a silent no-match).
    let hits = db.search_tasks_global("C:\\temp", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "path C:\\temp report");

    // `%` and `_` must stay literal, not act as wildcards.
    let hits = db.search_tasks_global("50%", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "progress 50% done");

    let hits = db.search_tasks_global("snake_case", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "snake_case name");

    // A bare `_` wildcard must not match arbitrary single characters.
    let hits = db.search_tasks_global("snakeXcase", 10).await.unwrap();
    assert!(hits.is_empty());
}
