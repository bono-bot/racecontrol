use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::Json;

use crate::privacy::audit::AuditEntry;
use crate::recognition::db;

use super::service::{self, EnrollmentState};
use super::types::{
    enrollment_status_with_threshold, CreatePersonRequest, PersonResponse, UpdatePersonRequest,
};

type SharedEnrollment = Arc<EnrollmentState>;

/// Build the enrollment router with all person CRUD + photo upload endpoints.
pub fn enrollment_router(state: SharedEnrollment) -> axum::Router {
    let photo_route = axum::Router::new()
        .route(
            "/api/v1/enrollment/persons/{id}/photos",
            post(upload_photo),
        )
        .layer(axum::extract::DefaultBodyLimit::max(
            state.config.body_limit_mb * 1024 * 1024,
        ))
        .with_state(Arc::clone(&state));

    let crud_routes = axum::Router::new()
        .route("/api/v1/enrollment/persons", post(create_person))
        .route("/api/v1/enrollment/persons", get(list_persons))
        .route("/api/v1/enrollment/persons/{id}", get(get_person))
        .route("/api/v1/enrollment/persons/{id}", put(update_person))
        .route("/api/v1/enrollment/persons/{id}", delete(delete_person))
        .with_state(state);

    crud_routes.merge(photo_route)
}

async fn create_person(
    State(state): State<SharedEnrollment>,
    Json(req): Json<CreatePersonRequest>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let name = req.name.clone();
    let role = req.role.clone();
    let phone = req.phone.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        db::create_tables(&conn).map_err(|e| format!("create_tables: {e}"))?;
        let id = db::insert_person(&conn, &name, &role, &phone)
            .map_err(|e| format!("insert_person: {e}"))?;
        let person = db::get_person(&conn, id)
            .map_err(|e| format!("get_person: {e}"))?
            .ok_or_else(|| "person not found after insert".to_string())?;
        Ok(person)
    })
    .await;

    match result {
        Ok(Ok(person)) => {
            let min_complete = state.config.min_embeddings_complete;
            state.audit.log(AuditEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                action: "person_enrolled".to_string(),
                person_id: Some(person.id.to_string()),
                accessor: "api:enrollment".to_string(),
                details: Some(format!("name={}, role={}", person.name, person.role)),
            });

            (
                StatusCode::CREATED,
                Json(PersonResponse {
                    id: person.id,
                    name: person.name,
                    role: person.role,
                    phone: person.phone,
                    embedding_count: 0,
                    enrollment_status: enrollment_status_with_threshold(0, min_complete)
                        .to_string(),
                    created_at: person.created_at,
                    updated_at: person.updated_at,
                }),
            )
                .into_response()
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e.to_string()})),
        )
            .into_response(),
    }
}

async fn list_persons(State(state): State<SharedEnrollment>) -> impl IntoResponse {
    let db_path = state.db_path.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        let persons =
            db::list_persons(&conn).map_err(|e| format!("list_persons: {e}"))?;
        let mut responses = Vec::with_capacity(persons.len());
        for p in persons {
            let count = db::embedding_count(&conn, p.id)
                .map_err(|e| format!("embedding_count: {e}"))?;
            responses.push((p, count));
        }
        Ok(responses)
    })
    .await;

    match result {
        Ok(Ok(persons)) => {
            let min_complete = state.config.min_embeddings_complete;
            let resp: Vec<PersonResponse> = persons
                .into_iter()
                .map(|(p, count)| PersonResponse {
                    id: p.id,
                    name: p.name,
                    role: p.role,
                    phone: p.phone,
                    embedding_count: count,
                    enrollment_status: enrollment_status_with_threshold(count, min_complete)
                        .to_string(),
                    created_at: p.created_at,
                    updated_at: p.updated_at,
                })
                .collect();
            Json(resp).into_response()
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e.to_string()})),
        )
            .into_response(),
    }
}

async fn get_person(
    State(state): State<SharedEnrollment>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        let person = db::get_person(&conn, id).map_err(|e| format!("get_person: {e}"))?;
        match person {
            Some(p) => {
                let count = db::embedding_count(&conn, id)
                    .map_err(|e| format!("embedding_count: {e}"))?;
                Ok(Some((p, count)))
            }
            None => Ok(None),
        }
    })
    .await;

    match result {
        Ok(Ok(Some((p, count)))) => {
            let min_complete = state.config.min_embeddings_complete;
            Json(PersonResponse {
                id: p.id,
                name: p.name,
                role: p.role,
                phone: p.phone,
                embedding_count: count,
                enrollment_status: enrollment_status_with_threshold(count, min_complete)
                    .to_string(),
                created_at: p.created_at,
                updated_at: p.updated_at,
            })
            .into_response()
        }
        Ok(Ok(None)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "person_not_found"})),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e.to_string()})),
        )
            .into_response(),
    }
}

async fn update_person(
    State(state): State<SharedEnrollment>,
    Path(id): Path<i64>,
    Json(req): Json<UpdatePersonRequest>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();
    let name = req.name.clone();
    let role = req.role.clone();
    let phone = req.phone.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        let updated = db::update_person(&conn, id, &name, &role, &phone)
            .map_err(|e| format!("update_person: {e}"))?;
        if !updated {
            return Ok(None);
        }
        let person = db::get_person(&conn, id)
            .map_err(|e| format!("get_person: {e}"))?
            .ok_or_else(|| "person not found after update".to_string())?;
        let count = db::embedding_count(&conn, id)
            .map_err(|e| format!("embedding_count: {e}"))?;
        Ok(Some((person, count)))
    })
    .await;

    match result {
        Ok(Ok(Some((p, count)))) => {
            let min_complete = state.config.min_embeddings_complete;
            state.audit.log(AuditEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                action: "person_updated".to_string(),
                person_id: Some(p.id.to_string()),
                accessor: "api:enrollment".to_string(),
                details: Some(format!("name={}, role={}", p.name, p.role)),
            });

            Json(PersonResponse {
                id: p.id,
                name: p.name,
                role: p.role,
                phone: p.phone,
                embedding_count: count,
                enrollment_status: enrollment_status_with_threshold(count, min_complete)
                    .to_string(),
                created_at: p.created_at,
                updated_at: p.updated_at,
            })
            .into_response()
        }
        Ok(Ok(None)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "person_not_found"})),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e.to_string()})),
        )
            .into_response(),
    }
}

async fn delete_person(
    State(state): State<SharedEnrollment>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let db_path = state.db_path.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<bool, String> {
        let conn =
            rusqlite::Connection::open(&db_path).map_err(|e| format!("db open: {e}"))?;
        db::delete_person(&conn, id).map_err(|e| format!("delete_person: {e}"))
    })
    .await;

    match result {
        Ok(Ok(true)) => {
            // Sync gallery
            state.gallery.remove_person(id).await;

            state.audit.log(AuditEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                action: "person_deleted".to_string(),
                person_id: Some(id.to_string()),
                accessor: "api:enrollment".to_string(),
                details: None,
            });

            StatusCode::NO_CONTENT.into_response()
        }
        Ok(Ok(false)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "person_not_found"})),
        )
            .into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "internal_error", "details": e.to_string()})),
        )
            .into_response(),
    }
}

async fn upload_photo(
    State(state): State<SharedEnrollment>,
    Path(id): Path<i64>,
    body: Bytes,
) -> impl IntoResponse {
    match service::process_photo(&state, id, &body).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => e.into_response(),
    }
}
