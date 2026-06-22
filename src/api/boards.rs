use poem::Result;
use poem_openapi::param::Path;
use poem_openapi::{OpenApi, payload::Json};

use super::dto::{BoardDto, CreateBoard, UpdateBoard};
use super::{ApiError, DeletedResponse};
use crate::db::Db;
use crate::domain::{Board, derive_board_key};

pub struct BoardApi {
    pub db: Db,
}

#[OpenApi]
impl BoardApi {
    /// List all boards.
    #[oai(path = "/boards", method = "get")]
    async fn list(&self) -> Result<Json<Vec<BoardDto>>> {
        let boards = self.db.list_boards().await.map_err(ApiError::from)?;
        Ok(Json(boards.into_iter().map(BoardDto::from).collect()))
    }

    /// Create a board. The key is derived from the name when omitted.
    #[oai(path = "/boards", method = "post")]
    async fn create(&self, body: Json<CreateBoard>) -> Result<Json<BoardDto>> {
        let body = body.0;
        let name = body.name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::bad_request("board name must not be empty").into());
        }
        let key = match body.key {
            Some(k) if !k.trim().is_empty() => k.trim().to_string(),
            _ => {
                let existing: Vec<String> = self
                    .db
                    .list_boards()
                    .await
                    .map_err(ApiError::from)?
                    .into_iter()
                    .map(|b| b.key)
                    .collect();
                derive_board_key(&name, &existing)
            }
        };
        let board = self
            .db
            .create_board(&name, &key)
            .await
            .map_err(ApiError::from)?;
        Ok(Json(board.into()))
    }

    /// Fetch a single board by id.
    #[oai(path = "/boards/:id", method = "get")]
    async fn get(&self, id: Path<i64>) -> Result<Json<BoardDto>> {
        Ok(Json(self.load(id.0).await?.into()))
    }

    /// Rename a board.
    #[oai(path = "/boards/:id", method = "patch")]
    async fn update(&self, id: Path<i64>, body: Json<UpdateBoard>) -> Result<Json<BoardDto>> {
        let id = id.0;
        self.load(id).await?;
        let name = body.0.name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::bad_request("board name must not be empty").into());
        }
        self.db
            .rename_board(id, &name)
            .await
            .map_err(ApiError::from)?;
        Ok(Json(self.load(id).await?.into()))
    }

    /// Delete a board (cascades to its columns, tasks and labels).
    #[oai(path = "/boards/:id", method = "delete")]
    async fn delete(&self, id: Path<i64>) -> Result<DeletedResponse> {
        let id = id.0;
        self.load(id).await?;
        self.db.delete_board(id).await.map_err(ApiError::from)?;
        Ok(DeletedResponse::NoContent)
    }
}

impl BoardApi {
    async fn load(&self, id: i64) -> Result<Board> {
        self.db
            .get_board(id)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::not_found(format!("board {id} not found")).into())
    }
}
