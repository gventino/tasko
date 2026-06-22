use poem::Result;
use poem_openapi::param::{Path, Query};
use poem_openapi::types::MaybeUndefined;
use poem_openapi::{OpenApi, payload::Json};

use super::dto::{ActivityDto, CreateTask, LabelDto, PatchTask, SetTaskLabels, TaskDto};
use super::{ApiError, DeletedResponse};
use crate::db::Db;
use crate::db::tasks::NewTask;
use crate::domain::{Priority, Task};

pub struct TaskApi {
    pub db: Db,
}

#[OpenApi]
impl TaskApi {
    /// List a board's tasks. Use `?top_level=true` to exclude subtasks, or
    /// `?parent_id=<id>` to fetch one task's children.
    #[oai(path = "/boards/:board_id/tasks", method = "get")]
    async fn list(
        &self,
        board_id: Path<i64>,
        parent_id: Query<Option<i64>>,
        top_level: Query<Option<bool>>,
    ) -> Result<Json<Vec<TaskDto>>> {
        self.ensure_board(board_id.0).await?;
        let tasks = self
            .db
            .tasks_for_board(board_id.0)
            .await
            .map_err(ApiError::from)?;
        let filtered = tasks.into_iter().filter(|t| {
            if let Some(pid) = parent_id.0 {
                t.parent_id == Some(pid)
            } else if top_level.0 == Some(true) {
                t.parent_id.is_none()
            } else {
                true
            }
        });
        Ok(Json(filtered.map(TaskDto::from).collect()))
    }

    /// Create a task, or a subtask when `parent_id` is set.
    #[oai(path = "/boards/:board_id/tasks", method = "post")]
    async fn create(&self, board_id: Path<i64>, body: Json<CreateTask>) -> Result<Json<TaskDto>> {
        let board_id = board_id.0;
        self.ensure_board(board_id).await?;
        let body = body.0;
        let title = body.title.trim().to_string();
        if title.is_empty() {
            return Err(ApiError::bad_request("task title must not be empty").into());
        }
        let column = self
            .db
            .get_column(body.column_id)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::not_found(format!("column {} not found", body.column_id)))?;
        if column.board_id != board_id {
            return Err(ApiError::bad_request("column does not belong to the board").into());
        }
        if let Some(pid) = body.parent_id {
            let parent = self
                .db
                .get_task(pid)
                .await
                .map_err(ApiError::from)?
                .ok_or_else(|| ApiError::not_found(format!("parent task {pid} not found")))?;
            if parent.board_id != board_id {
                return Err(
                    ApiError::bad_request("parent task does not belong to the board").into(),
                );
            }
        }
        let task = self
            .db
            .create_task(NewTask {
                board_id,
                column_id: body.column_id,
                parent_id: body.parent_id,
                title,
                description: body.description.unwrap_or_default(),
                priority: body.priority.map(Priority::from).unwrap_or_default(),
                due_date: body.due_date,
            })
            .await
            .map_err(ApiError::from)?;
        Ok(Json(task.into()))
    }

    /// Fetch a single task by id.
    #[oai(path = "/tasks/:id", method = "get")]
    async fn get(&self, id: Path<i64>) -> Result<Json<TaskDto>> {
        Ok(Json(self.load(id.0).await?.into()))
    }

    /// Patch any subset of a task's fields: content (title, description,
    /// priority, due_date), status (done) and/or position (column_id, position).
    #[oai(path = "/tasks/:id", method = "patch")]
    async fn update(&self, id: Path<i64>, body: Json<PatchTask>) -> Result<Json<TaskDto>> {
        let id = id.0;
        let current = self.load(id).await?;
        let body = body.0;

        let touches_content = body.title.is_some()
            || body.description.is_some()
            || body.priority.is_some()
            || !body.due_date.is_undefined();
        if touches_content {
            let title = body.title.unwrap_or_else(|| current.title.clone());
            let description = body
                .description
                .unwrap_or_else(|| current.description.clone());
            let priority = body
                .priority
                .map(Priority::from)
                .unwrap_or(current.priority);
            let due_date = match body.due_date {
                MaybeUndefined::Undefined => current.due_date,
                MaybeUndefined::Null => None,
                MaybeUndefined::Value(d) => Some(d),
            };
            self.db
                .update_task_content(id, &title, &description, priority, due_date)
                .await
                .map_err(ApiError::from)?;
        }
        if let Some(done) = body.done {
            self.db
                .set_task_done(id, done)
                .await
                .map_err(ApiError::from)?;
        }
        if body.column_id.is_some() || body.position.is_some() {
            let column_id = body.column_id.unwrap_or(current.column_id);
            let position = body.position.unwrap_or(current.position);
            self.db
                .move_task(id, column_id, position)
                .await
                .map_err(ApiError::from)?;
        }
        Ok(Json(self.load(id).await?.into()))
    }

    /// Delete a task (cascades to its subtasks).
    #[oai(path = "/tasks/:id", method = "delete")]
    async fn delete(&self, id: Path<i64>) -> Result<DeletedResponse> {
        let id = id.0;
        self.load(id).await?;
        self.db.delete_task(id).await.map_err(ApiError::from)?;
        Ok(DeletedResponse::NoContent)
    }

    /// List a task's direct subtasks.
    #[oai(path = "/tasks/:id/subtasks", method = "get")]
    async fn subtasks(&self, id: Path<i64>) -> Result<Json<Vec<TaskDto>>> {
        self.load(id.0).await?;
        let subs = self.db.subtasks(id.0).await.map_err(ApiError::from)?;
        Ok(Json(subs.into_iter().map(TaskDto::from).collect()))
    }

    /// List the labels attached to a task.
    #[oai(path = "/tasks/:id/labels", method = "get")]
    async fn get_labels(&self, id: Path<i64>) -> Result<Json<Vec<LabelDto>>> {
        self.load(id.0).await?;
        let labels = self
            .db
            .labels_for_task(id.0)
            .await
            .map_err(ApiError::from)?;
        Ok(Json(labels.into_iter().map(LabelDto::from).collect()))
    }

    /// Replace the full set of labels attached to a task.
    #[oai(path = "/tasks/:id/labels", method = "put")]
    async fn set_labels(
        &self,
        id: Path<i64>,
        body: Json<SetTaskLabels>,
    ) -> Result<Json<Vec<LabelDto>>> {
        let id = id.0;
        let task = self.load(id).await?;
        let board_labels = self
            .db
            .labels_for_board(task.board_id)
            .await
            .map_err(ApiError::from)?;
        for lid in &body.0.label_ids {
            if !board_labels.iter().any(|l| l.id == *lid) {
                return Err(
                    ApiError::bad_request(format!("label {lid} is not on this board")).into(),
                );
            }
        }
        self.db
            .set_task_labels(id, &body.0.label_ids)
            .await
            .map_err(ApiError::from)?;
        let labels = self.db.labels_for_task(id).await.map_err(ApiError::from)?;
        Ok(Json(labels.into_iter().map(LabelDto::from).collect()))
    }

    /// List a task's activity history (read-only).
    #[oai(path = "/tasks/:id/activities", method = "get")]
    async fn activities(&self, id: Path<i64>) -> Result<Json<Vec<ActivityDto>>> {
        self.load(id.0).await?;
        let activities = self
            .db
            .activities_for_task(id.0)
            .await
            .map_err(ApiError::from)?;
        Ok(Json(
            activities.into_iter().map(ActivityDto::from).collect(),
        ))
    }
}

impl TaskApi {
    async fn load(&self, id: i64) -> Result<Task> {
        self.db
            .get_task(id)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::not_found(format!("task {id} not found")).into())
    }

    async fn ensure_board(&self, board_id: i64) -> Result<()> {
        self.db
            .get_board(board_id)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::not_found(format!("board {board_id} not found")))?;
        Ok(())
    }
}
