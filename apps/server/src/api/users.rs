//! DFC Users and RBAC API routes.
//!
//! These routes are DFC-specific and not part of the upstream Wealthfolio API.

use std::sync::Arc;

use crate::{error::ApiResult, main_lib::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use wealthfolio_core::users::{NewUser, NewUserRole, User, UserRole};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListUsersQuery {
    active_only: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UserWithRolesResponse {
    #[serde(flatten)]
    user: User,
    roles: Vec<String>,
}

/// GET /api/dfc/users - List all users
async fn list_users(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListUsersQuery>,
) -> ApiResult<Json<Vec<User>>> {
    let active_only = query.active_only.unwrap_or(true);
    let users = state.user_repository.list_users(active_only)?;
    Ok(Json(users))
}

/// POST /api/dfc/users - Create a new user
async fn create_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<NewUser>,
) -> ApiResult<Json<User>> {
    let user = state.user_repository.create_user(payload)?;
    Ok(Json(user))
}

/// GET /api/dfc/users/:id - Get user by ID
async fn get_user(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<UserWithRolesResponse>> {
    let user = state
        .user_repository
        .get_user_by_id(&id)?
        .ok_or_else(|| crate::error::ApiError::NotFound)?;
    
    let roles = state.user_repository.get_user_roles(&id)?;
    let role_names: Vec<String> = roles.into_iter().map(|r| r.role).collect();
    
    Ok(Json(UserWithRolesResponse {
        user,
        roles: role_names,
    }))
}

/// POST /api/dfc/users/:id/roles - Assign a role to a user
async fn assign_role(
    Path(user_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AssignRoleRequest>,
) -> ApiResult<Json<UserRole>> {
    let new_role = NewUserRole {
        id: None,
        user_id,
        role: payload.role,
        created_by: payload.created_by,
    };
    let role = state.user_repository.assign_role(new_role)?;
    Ok(Json(role))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssignRoleRequest {
    role: String,
    created_by: Option<String>,
}

/// DELETE /api/dfc/users/:id/roles/:role - Remove a role from a user
async fn remove_role(
    Path((user_id, role)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> ApiResult<StatusCode> {
    state.user_repository.remove_role(&user_id, &role)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/dfc/users", get(list_users).post(create_user))
        .route("/dfc/users/{id}", get(get_user))
        .route("/dfc/users/{id}/roles", axum::routing::post(assign_role))
        .route(
            "/dfc/users/{id}/roles/{role}",
            axum::routing::delete(remove_role),
        )
}
