//! Checklist API server.
//!
//! The server exposes a thin HTTP layer over the SQLite-backed checklist store and keeps
//! the application architecture simple: storage in the core crate, HTTP handlers here,
//! and a static front-end in the UI crate.

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::{Method, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use checklist_core::{Checklist, ChecklistStore};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

#[derive(Parser, Debug)]
#[command(author, version, about = "Checklist server")]
struct Cli {
    #[arg(long, default_value = "./checklists.db")]
    database: PathBuf,
    #[arg(long, default_value = "127.0.0.1:3000")]
    bind: String,
}

/**
 * Shared application state carried through Axum routes.
 */
#[derive(Clone)]
struct AppState {
    store: Arc<Mutex<ChecklistStore>>,
}

/**
 * JSON payload used when creating a checklist from the form-based workflow.
 */
#[derive(Debug, Serialize, Deserialize)]
struct CreateChecklistRequest {
    title: String,
    description: String,
    items: Vec<String>,
}

/**
 * Query parameters for list/search endpoints.
 *
 * `limit` is intentionally capped server-side to keep responses bounded and predictable.
 */
#[derive(Debug, Serialize, Deserialize)]
struct SearchQuery {
    q: Option<String>,
    title: Option<String>,
    description: Option<String>,
    created_at: Option<String>,
    item: Option<String>,
    limit: Option<usize>,
}

fn swagger_ui_page() -> String {
    r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Checklist API Docs</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5.11.0/swagger-ui.css" />
    <style>
      body { margin: 0; background: #f5f7fb; }
      #swagger-ui { max-width: 1200px; margin: 20px auto; }
    </style>
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5.11.0/swagger-ui-bundle.js"></script>
    <script>
      window.onload = function () {
        SwaggerUIBundle({
          url: "/api-docs/openapi.json",
          dom_id: '#swagger-ui',
          deepLinking: true,
          presets: [SwaggerUIBundle.presets.apis, SwaggerUIBundle.SupportedSubmitMethods],
          layout: 'BaseLayout',
          theme: 'Flattop'
        });
      };
    </script>
  </body>
</html>
"##
    .to_string()
}

fn openapi_document() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "Checklist API",
            "version": "0.1.0",
            "description": "REST API for checklist storage, search, export, import, packing, and random access."
        },
        "paths": {
            "/api/checklists": {
                "get": {"summary": "List all checklists", "responses": {"200": {"description": "A list of checklists"}}},
                "post": {"summary": "Create a checklist", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/CreateChecklistRequest"}}}}, "responses": {"201": {"description": "Checklist created"}}}
            },
            "/api/checklists/{id}": {
                "get": {"summary": "Get a checklist by id", "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "integer"}}], "responses": {"200": {"description": "Checklist"}}},
                "delete": {"summary": "Delete a checklist by id", "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "integer"}}], "responses": {"204": {"description": "Deleted"}}}
            },
            "/api/checklists/search": {
                "get": {"summary": "Search checklists by title, description, or item text", "parameters": [
                    {"name": "q", "in": "query", "schema": {"type": "string"}, "required": false},
                    {"name": "title", "in": "query", "schema": {"type": "string"}, "required": false},
                    {"name": "description", "in": "query", "schema": {"type": "string"}, "required": false},
                    {"name": "created_at", "in": "query", "schema": {"type": "string"}, "required": false},
                    {"name": "item", "in": "query", "schema": {"type": "string"}, "required": false}
                ], "responses": {"200": {"description": "Search results"}}}
            },
            "/api/checklists/random": {
                "get": {"summary": "Get a random checklist", "responses": {"200": {"description": "A checklist"}}}
            },
            "/api/checklists/{id}/export.json": {
                "get": {"summary": "Export a checklist as JSON", "parameters": [{"name": "id", "in": "path", "required": true, "schema": {"type": "integer"}}], "responses": {"200": {"description": "Checklist JSON"}}}
            },
            "/api/checklists/template": {
                "get": {"summary": "Get a checklist JSON template", "responses": {"200": {"description": "Checklist template example", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/CreateChecklistRequest"}}}}}}
            },
            "/api/checklists/from-text": {
                "post": {"summary": "Create a checklist from a plain-text checklist string", "requestBody": {"required": true, "content": {"text/plain": {"schema": {"type": "string"}}}}, "responses": {"201": {"description": "Checklist created from text"}}}
            },
            "/api/checklists/import": {
                "post": {"summary": "Import a checklist from a JSON payload", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Checklist"}}}}, "responses": {"201": {"description": "Imported"}}}
            },
            "/api/checklists/pack": {
                "post": {"summary": "Pack a checklist into a UTF-8 encoded base64 string", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Checklist"}}}}, "responses": {"200": {"description": "Packed string"}}}
            },
            "/api/checklists/unpack": {
                "post": {"summary": "Unpack a checklist from a base64 string", "requestBody": {"required": true, "content": {"application/json": {"schema": {"type": "string"}}}}, "responses": {"200": {"description": "Checklist"}}}
            }
        },
        "components": {
            "schemas": {
                "CreateChecklistRequest": {
                    "type": "object",
                    "required": ["title", "description", "items"],
                    "properties": {
                        "title": {"type": "string"},
                        "description": {"type": "string"},
                        "items": {"type": "array", "items": {"type": "string"}}
                    }
                },
                "Checklist": {
                    "type": "object",
                    "required": ["id", "title", "description", "created_at", "items"],
                    "properties": {
                        "id": {"type": "integer", "format": "int64"},
                        "title": {"type": "string"},
                        "description": {"type": "string"},
                        "created_at": {"type": "string", "format": "date-time"},
                        "items": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/ChecklistItem"}
                        },
                        "source_id": {"type": ["integer", "null"]}
                    }
                },
                "ChecklistItem": {
                    "type": "object",
                    "required": ["id", "step"],
                    "properties": {
                        "id": {"type": "integer", "format": "int64"},
                        "step": {"type": "string"}
                    }
                }
            }
        }
    })
}

/// Lists the most recent checklists, bounded by the server limit policy.
async fn list_checklists(
    State(state): State<AppState>,
    query: Query<SearchQuery>,
) -> Result<Json<Vec<Checklist>>, AppError> {
    let limit = query
        .limit
        .unwrap_or(checklist_core::DEFAULT_RESULT_LIMIT)
        .min(checklist_core::DEFAULT_RESULT_LIMIT);
    let checklists = state
        .store
        .lock()
        .unwrap()
        .list_checklists_limited(Some(limit))?;
    Ok(Json(checklists))
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("store error: {0}")]
    Store(#[from] checklist_core::ChecklistError),
    #[error("bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self {
            AppError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
        };
        (status, self.to_string()).into_response()
    }
}

async fn create_checklist_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateChecklistRequest>,
) -> Result<(StatusCode, Json<Checklist>), AppError> {
    let checklist = Checklist::new(
        payload.title.trim(),
        payload.description.trim(),
        payload.items,
        "0.0.0.0",
    );
    let created = state.store.lock().unwrap().create_checklist(checklist)?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn get_checklist_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Result<Json<Checklist>, AppError> {
    let checklist = state
        .store
        .lock()
        .unwrap()
        .get_checklist(id)?
        .ok_or_else(|| AppError::BadRequest(format!("checklist {} not found", id)))?;
    Ok(Json(checklist))
}

/// Handles search requests. Empty queries fall back to a capped list response.
async fn search_checklists_handler(
    State(state): State<AppState>,
    query: Query<SearchQuery>,
) -> Result<Json<Vec<Checklist>>, AppError> {
    let q = query
        .q
        .clone()
        .or_else(|| query.title.clone())
        .or_else(|| query.description.clone())
        .or_else(|| query.item.clone())
        .unwrap_or_default();
    let limit = query
        .limit
        .unwrap_or(checklist_core::DEFAULT_RESULT_LIMIT)
        .min(checklist_core::DEFAULT_RESULT_LIMIT);

    if q.trim().is_empty() {
        let checklists = state
            .store
            .lock()
            .unwrap()
            .list_checklists_limited(Some(limit))?;
        return Ok(Json(checklists));
    }

    let results = state
        .store
        .lock()
        .unwrap()
        .search_by_term_limited(&q, Some(limit))?;
    Ok(Json(results))
}

async fn random_checklist_handler(
    State(state): State<AppState>,
) -> Result<Json<Checklist>, AppError> {
    let checklist = state
        .store
        .lock()
        .unwrap()
        .random_checklist()?
        .ok_or_else(|| AppError::BadRequest("no checklists available".to_string()))?;
    Ok(Json(checklist))
}

async fn export_checklist_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Result<Response, AppError> {
    let checklist = state
        .store
        .lock()
        .unwrap()
        .get_checklist(id)?
        .ok_or_else(|| AppError::BadRequest(format!("checklist {} not found", id)))?;
    let json = checklist.to_json()?;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        json,
    )
        .into_response())
}

async fn import_checklist_handler(
    State(state): State<AppState>,
    Json(payload): Json<Checklist>,
) -> Result<(StatusCode, Json<Checklist>), AppError> {
    let imported = state.store.lock().unwrap().create_checklist(payload)?;
    Ok((StatusCode::CREATED, Json(imported)))
}

async fn template_checklist_handler() -> Json<CreateChecklistRequest> {
    Json(CreateChecklistRequest {
        title: "Example checklist title".to_string(),
        description: "Example checklist description".to_string(),
        items: vec!["First step".to_string(), "Second step".to_string()],
    })
}

async fn create_checklist_from_text_handler(
    State(state): State<AppState>,
    text: String,
) -> Result<(StatusCode, Json<Checklist>), AppError> {
    let checklist = Checklist::from_text(&text)?;
    let created = state.store.lock().unwrap().create_checklist(checklist)?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn pack_checklist_handler(
    State(_state): State<AppState>,
    Json(payload): Json<Checklist>,
) -> Result<String, AppError> {
    Ok(payload.to_packed_string()?)
}

async fn unpack_checklist_handler(
    State(_state): State<AppState>,
    Json(payload): Json<String>,
) -> Result<Json<Checklist>, AppError> {
    let checklist = Checklist::from_packed_string(&payload)?;
    Ok(Json(checklist))
}

async fn delete_checklist_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Result<StatusCode, AppError> {
    let exists = state.store.lock().unwrap().get_checklist(id)?.is_some();
    if !exists {
        return Err(AppError::BadRequest(format!("checklist {} not found", id)));
    }
    state.store.lock().unwrap().delete_checklist(id)?;
    Ok(StatusCode::NO_CONTENT)
}

fn app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route(
            "/api/checklists",
            get(list_checklists).post(create_checklist_handler),
        )
        .route("/api/checklists/search", get(search_checklists_handler))
        .route("/api/checklists/random", get(random_checklist_handler))
        .route(
            "/api/checklists/{id}",
            get(get_checklist_handler).delete(delete_checklist_handler),
        )
        .route(
            "/api/checklists/{id}/export.json",
            get(export_checklist_handler),
        )
        .route("/api/checklists/template", get(template_checklist_handler))
        .route(
            "/api/checklists/from-text",
            post(create_checklist_from_text_handler),
        )
        .route("/api/checklists/import", post(import_checklist_handler))
        .route("/api/checklists/pack", post(pack_checklist_handler))
        .route("/api/checklists/unpack", post(unpack_checklist_handler))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let store = Arc::new(Mutex::new(
        ChecklistStore::open(&cli.database).expect("failed to open checklist database"),
    ));

    let app = app(AppState {
        store: store.clone(),
    });
    let addr: SocketAddr = cli.bind.parse().expect("invalid bind address");

    let docs = openapi_document();
    let docs_for_json = docs.clone();
    let router = app
        .route("/api/docs", get(|| async { Html(swagger_ui_page()) }))
        .route(
            "/api-docs/openapi.json",
            get(move || async move { Json(docs_for_json) }),
        );

    println!("Checklist server listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind to socket");
    axum::serve(listener, router).await.expect("server failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use checklist_core::ChecklistStore;
    use tempfile::NamedTempFile;
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn list_checklists_route_works() {
        let file = NamedTempFile::new().unwrap();
        let store = Arc::new(Mutex::new(ChecklistStore::open(file.path()).unwrap()));
        let app = app(AppState { store });

        let request = Request::builder()
            .method("GET")
            .uri("/api/checklists")
            .header("Origin", "http://127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "*"
        );
    }
}
