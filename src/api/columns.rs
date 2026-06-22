use poem::Result;
use poem_openapi::param::Path;
use poem_openapi::types::MaybeUndefined;
use poem_openapi::{OpenApi, payload::Json};

use super::dto::{ColumnDto, CreateColumn, UpdateColumn};
use super::{ApiError, DeletedResponse, IntoApiResult};
use crate::db::Db;
use crate::domain::Column;

pub struct ColumnApi {
    pub db: Db,
}

#[OpenApi]
impl ColumnApi {
    /// List a board's columns, left to right.
    #[oai(path = "/boards/:board_id/columns", method = "get")]
    async fn list(&self, board_id: Path<i64>) -> Result<Json<Vec<ColumnDto>>> {
        super::ensure_board(&self.db, board_id.0).await?;
        let columns = self.db.columns_for_board(board_id.0).await.api()?;
        Ok(Json(columns.into_iter().map(ColumnDto::from).collect()))
    }

    /// Create a column at the end of a board.
    #[oai(path = "/boards/:board_id/columns", method = "post")]
    async fn create(
        &self,
        board_id: Path<i64>,
        body: Json<CreateColumn>,
    ) -> Result<super::Created<ColumnDto>> {
        super::ensure_board(&self.db, board_id.0).await?;
        let name = body.0.name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::bad_request("column name must not be empty").into());
        }
        let column = self.db.create_column(board_id.0, &name).await.api()?;
        let location = format!("/columns/{}", column.id);
        Ok(super::Created::Created(Json(column.into()), location))
    }

    /// Fetch a single column by id.
    #[oai(path = "/columns/:id", method = "get")]
    async fn get(&self, id: Path<i64>) -> Result<Json<ColumnDto>> {
        Ok(Json(self.load(id.0).await?.into()))
    }

    /// Update a column's name and/or WIP limit.
    #[oai(path = "/columns/:id", method = "patch")]
    async fn update(&self, id: Path<i64>, body: Json<UpdateColumn>) -> Result<Json<ColumnDto>> {
        let id = id.0;
        self.load(id).await?;
        let body = body.0;
        if let Some(name) = body.name {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(ApiError::bad_request("column name must not be empty").into());
            }
            self.db.rename_column(id, &name).await.api()?;
        }
        match body.wip_limit {
            MaybeUndefined::Undefined => {}
            MaybeUndefined::Null => self.db.set_wip_limit(id, None).await.api()?,
            MaybeUndefined::Value(v) => self.db.set_wip_limit(id, Some(v)).await.api()?,
        }
        Ok(Json(self.load(id).await?.into()))
    }

    /// Delete a column (cascades to its tasks).
    #[oai(path = "/columns/:id", method = "delete")]
    async fn delete(&self, id: Path<i64>) -> Result<DeletedResponse> {
        let id = id.0;
        self.load(id).await?;
        self.db.delete_column(id).await.api()?;
        Ok(DeletedResponse::NoContent)
    }
}

impl ColumnApi {
    async fn load(&self, id: i64) -> Result<Column> {
        Ok(super::found(
            self.db.get_column(id).await.api()?,
            "column",
            id,
        )?)
    }
}
