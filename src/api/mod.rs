//! HTTP REST API mode (`tasko serve`). A poem + poem-openapi server that
//! exposes CRUD over the application's entities and a Swagger UI, reusing the
//! existing [`Db`] layer.

mod boards;
mod columns;
mod dto;
mod labels;
mod tasks;

use anyhow::Result;
use poem::http::StatusCode;
use poem::listener::TcpListener;
use poem::{Endpoint, EndpointExt, Response, Route, Server};
use poem_openapi::{ApiResponse, Object, OpenApi, OpenApiService, payload::Json};

use crate::db::Db;
use boards::BoardApi;
use columns::ColumnApi;
use labels::LabelApi;
use tasks::TaskApi;

/// Error returned by API handlers; rendered as `{ "error": "..." }` with an
/// appropriate HTTP status.
#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Internal(anyhow::Error),
}

impl ApiError {
    pub(crate) fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub(crate) fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    fn message(&self) -> String {
        match self {
            Self::NotFound(m) | Self::BadRequest(m) => m.clone(),
            Self::Internal(_) => "internal server error".to_string(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl std::error::Error for ApiError {}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::Internal(e)
    }
}

pub(crate) trait IntoApiResult<T> {
    fn api(self) -> std::result::Result<T, ApiError>;
}

impl<T> IntoApiResult<T> for anyhow::Result<T> {
    fn api(self) -> std::result::Result<T, ApiError> {
        self.map_err(ApiError::Internal)
    }
}

impl poem::error::ResponseError for ApiError {
    fn status(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(e) => internal_status(e),
        }
    }

    fn as_response(&self) -> Response {
        let body = serde_json::json!({ "error": self.message() }).to_string();
        Response::builder()
            .status(self.status())
            .header("content-type", "application/json")
            .body(body)
    }
}

/// Surface a 409 for SQLite constraint violations; everything else is a 500.
fn internal_status(e: &anyhow::Error) -> StatusCode {
    if let Some(sqlx::Error::Database(db)) = e.downcast_ref::<sqlx::Error>() {
        use sqlx::error::ErrorKind;
        if matches!(
            db.kind(),
            ErrorKind::UniqueViolation | ErrorKind::ForeignKeyViolation
        ) {
            return StatusCode::CONFLICT;
        }
    }
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn render_error(err: poem::Error) -> poem::Response {
    use poem::IntoResponse;

    let status = err.status();
    let message = match err.downcast_ref::<ApiError>() {
        Some(ApiError::Internal(e)) => {
            eprintln!("internal API error: {e:#}");
            "internal server error".to_string()
        }
        Some(other) => other.to_string(),
        None => err.to_string(),
    };

    poem::web::Json(serde_json::json!({ "error": message }))
        .with_status(status)
        .into_response()
}

/// Empty 204 response shared by all delete operations.
#[derive(ApiResponse)]
pub enum DeletedResponse {
    /// Resource deleted.
    #[oai(status = 204)]
    NoContent,
}

#[derive(ApiResponse)]
pub(crate) enum Created<T: poem_openapi::types::ToJSON> {
    #[oai(status = 201)]
    Created(Json<T>, #[oai(header = "Location")] String),
}

pub(crate) fn found<T>(opt: Option<T>, what: &str, id: i64) -> std::result::Result<T, ApiError> {
    opt.ok_or_else(|| ApiError::not_found(format!("{what} {id} not found")))
}

pub(crate) async fn ensure_board(
    db: &crate::db::Db,
    board_id: i64,
) -> std::result::Result<(), ApiError> {
    if db.get_board(board_id).await.api()?.is_none() {
        return Err(ApiError::not_found(format!("board {board_id} not found")));
    }
    Ok(())
}

#[derive(Object)]
struct Health {
    status: String,
}

struct MetaApi;

#[OpenApi]
impl MetaApi {
    /// Liveness probe.
    #[oai(path = "/health", method = "get")]
    async fn health(&self) -> Json<Health> {
        Json(Health {
            status: "ok".to_string(),
        })
    }
}

/// Assemble the poem application (OpenAPI routes + Swagger UI + spec).
fn build_app(db: Db, port: u16) -> impl Endpoint {
    let apis = (
        MetaApi,
        BoardApi { db: db.clone() },
        ColumnApi { db: db.clone() },
        TaskApi { db: db.clone() },
        LabelApi { db },
    );
    let service = OpenApiService::new(apis, "tasko", env!("CARGO_PKG_VERSION"))
        .server(format!("http://127.0.0.1:{port}"));
    let swagger = service.swagger_ui();
    let spec = service.spec_endpoint();
    Route::new()
        .nest("/swagger-ui", swagger)
        .at("/openapi.json", spec)
        .nest("/", service)
        .catch_all_error(render_error)
}

/// Run the HTTP REST API server, bound to localhost on `port`.
pub async fn serve(db: Db, port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    println!("tasko API listening on http://{addr}  (Swagger UI: http://{addr}/swagger-ui)");
    Server::new(TcpListener::bind(addr))
        .run(build_app(db, port))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
