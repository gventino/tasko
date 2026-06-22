use poem::Result;
use poem_openapi::param::{Path, Query};
use poem_openapi::types::MaybeUndefined;
use poem_openapi::{OpenApi, payload::Json};

use super::dto::{ActivityDto, CreateTask, LabelDto, PatchTask, SetTaskLabels, TaskDto};
use super::{ApiError, DeletedResponse, IntoApiResult};
use crate::db::Db;
use crate::db::tasks::{NewTask, TaskPatch};
use crate::domain::Priority;

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
        super::ensure_board(&self.db, board_id.0).await?;
        let tasks = self.db.tasks_for_board(board_id.0).await.api()?;
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
    async fn create(
        &self,
        board_id: Path<i64>,
        body: Json<CreateTask>,
    ) -> Result<super::Created<TaskDto>> {
        let board_id = board_id.0;
        super::ensure_board(&self.db, board_id).await?;
        let body = body.0;
        let title = body.title.trim().to_string();
        if title.is_empty() {
            return Err(ApiError::bad_request("task title must not be empty").into());
        }
        let column = super::found(
            self.db.get_column(body.column_id).await.api()?,
            "column",
            body.column_id,
        )?;
        if column.board_id != board_id {
            return Err(ApiError::bad_request("column does not belong to the board").into());
        }
        if let Some(pid) = body.parent_id {
            let parent = super::found(self.db.get_task(pid).await.api()?, "parent task", pid)?;
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
            .api()?;
        let location = format!("/tasks/{}", task.id);
        Ok(super::Created::Created(Json(task.into()), location))
    }

    /// Fetch a single task by id.
    #[oai(path = "/tasks/:id", method = "get")]
    async fn get(&self, id: Path<i64>) -> Result<Json<TaskDto>> {
        let id = id.0;
        let task = super::found(self.db.get_task(id).await.api()?, "task", id)?;
        Ok(Json(task.into()))
    }

    /// Patch any subset of a task's fields: content (title, description,
    /// priority, due_date), status (done) and/or position (column_id, position).
    #[oai(path = "/tasks/:id", method = "patch")]
    async fn update(&self, id: Path<i64>, body: Json<PatchTask>) -> Result<Json<TaskDto>> {
        let id = id.0;
        let current = super::found(self.db.get_task(id).await.api()?, "task", id)?;
        let body = body.0;

        if let Some(column_id) = body.column_id {
            let column = super::found(
                self.db.get_column(column_id).await.api()?,
                "column",
                column_id,
            )?;
            if column.board_id != current.board_id {
                return Err(
                    ApiError::bad_request("column does not belong to the task's board").into(),
                );
            }
        }

        let move_to = if body.column_id.is_some() || body.position.is_some() {
            Some((
                body.column_id.unwrap_or(current.column_id),
                body.position.unwrap_or(current.position),
            ))
        } else {
            None
        };
        let due_date = match body.due_date {
            MaybeUndefined::Undefined => None,
            MaybeUndefined::Null => Some(None),
            MaybeUndefined::Value(d) => Some(Some(d)),
        };
        let patch = TaskPatch {
            title: body.title,
            description: body.description,
            priority: body.priority.map(Priority::from),
            due_date,
            done: body.done,
            move_to,
        };
        let task = super::found(self.db.patch_task(id, patch).await.api()?, "task", id)?;
        Ok(Json(task.into()))
    }

    /// Delete a task (cascades to its subtasks).
    #[oai(path = "/tasks/:id", method = "delete")]
    async fn delete(&self, id: Path<i64>) -> Result<DeletedResponse> {
        let id = id.0;
        super::found(self.db.get_task(id).await.api()?, "task", id)?;
        self.db.delete_task(id).await.api()?;
        Ok(DeletedResponse::NoContent)
    }

    /// List a task's direct subtasks.
    #[oai(path = "/tasks/:id/subtasks", method = "get")]
    async fn subtasks(&self, id: Path<i64>) -> Result<Json<Vec<TaskDto>>> {
        super::found(self.db.get_task(id.0).await.api()?, "task", id.0)?;
        let subs = self.db.subtasks(id.0).await.api()?;
        Ok(Json(subs.into_iter().map(TaskDto::from).collect()))
    }

    /// List the labels attached to a task.
    #[oai(path = "/tasks/:id/labels", method = "get")]
    async fn get_labels(&self, id: Path<i64>) -> Result<Json<Vec<LabelDto>>> {
        super::found(self.db.get_task(id.0).await.api()?, "task", id.0)?;
        let labels = self.db.labels_for_task(id.0).await.api()?;
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
        let task = super::found(self.db.get_task(id).await.api()?, "task", id)?;
        let board_labels = self.db.labels_for_board(task.board_id).await.api()?;
        let mut ids = body.0.label_ids;
        ids.sort_unstable();
        ids.dedup();
        for lid in &ids {
            if !board_labels.iter().any(|l| l.id == *lid) {
                return Err(
                    ApiError::bad_request(format!("label {lid} is not on this board")).into(),
                );
            }
        }
        self.db.set_task_labels(id, &ids).await.api()?;
        let labels = self.db.labels_for_task(id).await.api()?;
        Ok(Json(labels.into_iter().map(LabelDto::from).collect()))
    }

    /// List a task's activity history (read-only).
    #[oai(path = "/tasks/:id/activities", method = "get")]
    async fn activities(&self, id: Path<i64>) -> Result<Json<Vec<ActivityDto>>> {
        super::found(self.db.get_task(id.0).await.api()?, "task", id.0)?;
        let activities = self.db.activities_for_task(id.0).await.api()?;
        Ok(Json(
            activities.into_iter().map(ActivityDto::from).collect(),
        ))
    }
}
