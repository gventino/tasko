use poem::Endpoint;
use poem::http::StatusCode;
use poem::test::TestClient;
use serde_json::json;

use crate::db::Db;

async fn test_client() -> TestClient<impl Endpoint> {
    let db = Db::connect_in_memory().await.unwrap();
    TestClient::new(super::build_app(db, 0))
}

async fn create_board<E: Endpoint>(cli: &TestClient<E>, name: &str) -> i64 {
    let resp = cli
        .post("/boards")
        .body_json(&json!({ "name": name }))
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.json().await.value().object().get("id").i64()
}

async fn first_column_id<E: Endpoint>(cli: &TestClient<E>, board_id: i64) -> i64 {
    let resp = cli.get(format!("/boards/{board_id}/columns")).send().await;
    resp.assert_status_is_ok();
    resp.json()
        .await
        .value()
        .array()
        .get(0)
        .object()
        .get("id")
        .i64()
}

#[tokio::test]
async fn health_reports_ok() {
    let cli = test_client().await;
    let resp = cli.get("/health").send().await;
    resp.assert_status_is_ok();
    resp.json()
        .await
        .value()
        .object()
        .get("status")
        .assert_string("ok");
}

#[tokio::test]
async fn board_crud_flow() {
    let cli = test_client().await;

    // Create derives a key and seeds the three default columns.
    let resp = cli
        .post("/boards")
        .body_json(&json!({ "name": "Main Board" }))
        .send()
        .await;
    resp.assert_status_is_ok();
    let created = resp.json().await;
    let id = created.value().object().get("id").i64();
    created.value().object().get("key").assert_string("MB");

    let resp = cli.get(format!("/boards/{id}/columns")).send().await;
    resp.assert_status_is_ok();
    resp.json().await.value().array().assert_len(3);

    // Rename.
    let resp = cli
        .patch(format!("/boards/{id}"))
        .body_json(&json!({ "name": "Renamed" }))
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.json()
        .await
        .value()
        .object()
        .get("name")
        .assert_string("Renamed");

    // List.
    let resp = cli.get("/boards").send().await;
    resp.assert_status_is_ok();
    resp.json().await.value().array().assert_len(1);

    // Delete then 404.
    cli.delete(format!("/boards/{id}"))
        .send()
        .await
        .assert_status(StatusCode::NO_CONTENT);
    cli.get(format!("/boards/{id}"))
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn board_validation_errors() {
    let cli = test_client().await;
    cli.get("/boards/9999")
        .send()
        .await
        .assert_status(StatusCode::NOT_FOUND);
    cli.post("/boards")
        .body_json(&json!({ "name": "   " }))
        .send()
        .await
        .assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn column_crud_flow() {
    let cli = test_client().await;
    let board = create_board(&cli, "Columns").await;

    let resp = cli
        .post(format!("/boards/{board}/columns"))
        .body_json(&json!({ "name": "Backlog" }))
        .send()
        .await;
    resp.assert_status_is_ok();
    let col = resp.json().await.value().object().get("id").i64();

    // Set then clear the WIP limit.
    let resp = cli
        .patch(format!("/columns/{col}"))
        .body_json(&json!({ "wip_limit": 5 }))
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.json()
        .await
        .value()
        .object()
        .get("wip_limit")
        .assert_i64(5);

    let resp = cli
        .patch(format!("/columns/{col}"))
        .body_json(&json!({ "wip_limit": null }))
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.json()
        .await
        .value()
        .object()
        .get("wip_limit")
        .assert_null();

    cli.delete(format!("/columns/{col}"))
        .send()
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn task_flow_with_subtasks_labels_and_activities() {
    let cli = test_client().await;
    let board = create_board(&cli, "Work").await;
    let column = first_column_id(&cli, board).await;

    // Create a task.
    let resp = cli
        .post(format!("/boards/{board}/tasks"))
        .body_json(&json!({ "column_id": column, "title": "Task A", "priority": "high" }))
        .send()
        .await;
    resp.assert_status_is_ok();
    let task = resp.json().await;
    let task_id = task.value().object().get("id").i64();
    task.value().object().get("priority").assert_string("high");

    // Patch: mark done.
    let resp = cli
        .patch(format!("/tasks/{task_id}"))
        .body_json(&json!({ "done": true }))
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.json()
        .await
        .value()
        .object()
        .get("done")
        .assert_bool(true);

    // Add a subtask and list it.
    let resp = cli
        .post(format!("/boards/{board}/tasks"))
        .body_json(&json!({ "column_id": column, "parent_id": task_id, "title": "Subtask" }))
        .send()
        .await;
    resp.assert_status_is_ok();
    let resp = cli.get(format!("/tasks/{task_id}/subtasks")).send().await;
    resp.assert_status_is_ok();
    resp.json().await.value().array().assert_len(1);

    // Attach a label.
    let resp = cli
        .post(format!("/boards/{board}/labels"))
        .body_json(&json!({ "name": "bug", "color": 1 }))
        .send()
        .await;
    resp.assert_status_is_ok();
    let label_id = resp.json().await.value().object().get("id").i64();

    let resp = cli
        .put(format!("/tasks/{task_id}/labels"))
        .body_json(&json!({ "label_ids": [label_id] }))
        .send()
        .await;
    resp.assert_status_is_ok();
    resp.json().await.value().array().assert_len(1);

    // Activity history records at least the creation event.
    let resp = cli.get(format!("/tasks/{task_id}/activities")).send().await;
    resp.assert_status_is_ok();
    assert!(!resp.json().await.value().array().is_empty());

    // Delete.
    cli.delete(format!("/tasks/{task_id}"))
        .send()
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn label_crud_flow() {
    let cli = test_client().await;
    let board = create_board(&cli, "Labels").await;

    let resp = cli
        .post(format!("/boards/{board}/labels"))
        .body_json(&json!({ "name": "feature" }))
        .send()
        .await;
    resp.assert_status_is_ok();
    let id = resp.json().await.value().object().get("id").i64();

    let resp = cli
        .patch(format!("/labels/{id}"))
        .body_json(&json!({ "name": "feat", "color": 3 }))
        .send()
        .await;
    resp.assert_status_is_ok();
    let updated = resp.json().await;
    updated.value().object().get("name").assert_string("feat");
    updated.value().object().get("color").assert_i64(3);

    let resp = cli.get(format!("/boards/{board}/labels")).send().await;
    resp.assert_status_is_ok();
    resp.json().await.value().array().assert_len(1);

    cli.delete(format!("/labels/{id}"))
        .send()
        .await
        .assert_status(StatusCode::NO_CONTENT);
}
