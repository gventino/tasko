use poem::Result;
use poem_openapi::param::Path;
use poem_openapi::{OpenApi, payload::Json};

use super::dto::{CreateLabel, LabelDto, UpdateLabel};
use super::{ApiError, DeletedResponse, IntoApiResult};
use crate::db::Db;
use crate::domain::Label;

pub struct LabelApi {
    pub db: Db,
}

#[OpenApi]
impl LabelApi {
    /// List a board's labels.
    #[oai(path = "/boards/:board_id/labels", method = "get")]
    async fn list(&self, board_id: Path<i64>) -> Result<Json<Vec<LabelDto>>> {
        super::ensure_board(&self.db, board_id.0).await?;
        let labels = self.db.labels_for_board(board_id.0).await.api()?;
        Ok(Json(labels.into_iter().map(LabelDto::from).collect()))
    }

    /// Create a label on a board.
    #[oai(path = "/boards/:board_id/labels", method = "post")]
    async fn create(
        &self,
        board_id: Path<i64>,
        body: Json<CreateLabel>,
    ) -> Result<super::Created<LabelDto>> {
        super::ensure_board(&self.db, board_id.0).await?;
        let body = body.0;
        let name = body.name.trim().to_string();
        if name.is_empty() {
            return Err(ApiError::bad_request("label name must not be empty").into());
        }
        let color = body.color.unwrap_or(0);
        let label = self.db.create_label(board_id.0, &name, color).await.api()?;
        let location = format!("/labels/{}", label.id);
        Ok(super::Created::Created(Json(label.into()), location))
    }

    /// Fetch a single label by id.
    #[oai(path = "/labels/:id", method = "get")]
    async fn get(&self, id: Path<i64>) -> Result<Json<LabelDto>> {
        Ok(Json(self.load(id.0).await?.into()))
    }

    /// Update a label's name and/or color.
    #[oai(path = "/labels/:id", method = "patch")]
    async fn update(&self, id: Path<i64>, body: Json<UpdateLabel>) -> Result<Json<LabelDto>> {
        let id = id.0;
        let current = self.load(id).await?;
        let body = body.0;
        let name = match body.name {
            Some(n) => {
                let n = n.trim().to_string();
                if n.is_empty() {
                    return Err(ApiError::bad_request("label name must not be empty").into());
                }
                n
            }
            None => current.name.clone(),
        };
        let color = body.color.unwrap_or(current.color);
        self.db.update_label(id, &name, color).await.api()?;
        Ok(Json(self.load(id).await?.into()))
    }

    /// Delete a label (removes it from any tasks it was attached to).
    #[oai(path = "/labels/:id", method = "delete")]
    async fn delete(&self, id: Path<i64>) -> Result<DeletedResponse> {
        let id = id.0;
        self.load(id).await?;
        self.db.delete_label(id).await.api()?;
        Ok(DeletedResponse::NoContent)
    }
}

impl LabelApi {
    async fn load(&self, id: i64) -> Result<Label> {
        Ok(super::found(
            self.db.get_label(id).await.api()?,
            "label",
            id,
        )?)
    }
}
