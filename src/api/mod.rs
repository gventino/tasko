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
use poem::{Endpoint, Response, Route, Server};
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
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    fn message(&self) -> String {
        match self {
            Self::NotFound(m) | Self::BadRequest(m) => m.clone(),
            Self::Internal(e) => e.to_string(),
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
            ErrorKind::UniqueViolation
                | ErrorKind::ForeignKeyViolation
                | ErrorKind::NotNullViolation
                | ErrorKind::CheckViolation
        ) {
            return StatusCode::CONFLICT;
        }
    }
    StatusCode::INTERNAL_SERVER_ERROR
}

/// Empty 204 response shared by all delete operations.
#[derive(ApiResponse)]
pub enum DeletedResponse {
    /// Resource deleted.
    #[oai(status = 204)]
    NoContent,
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
