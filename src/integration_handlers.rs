use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    discord::DiscordConfig,
    github::GitHubOAuthConfig,
    slack::SlackOAuthConfig,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIntegrationRequest {
    pub access_token: String,
    pub owner: String,
    pub repository: String,
    pub issue_title_template: Option<String>,
    pub issue_body_template: Option<String>,
    pub auto_create_issues: Option<bool>,
    pub pr_comment_enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscordIntegrationRequest {
    pub webhook_url: String,
    pub bot_name: Option<String>,
    pub avatar_url: Option<String>,
    pub embed_enabled: Option<bool>,
    pub thread_support: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SlackIntegrationRequest {
    pub webhook_url: Option<String>,
    pub bot_token: Option<String>,
    pub channel: String,
    pub block_kit_enabled: Option<bool>,
    pub thread_support: Option<bool>,
    pub user_mentions_enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TelegramIntegrationRequest {
    pub bot_token: String,
    pub chat_id: String,
    pub webhook_enabled: Option<bool>,
    pub webhook_url: Option<String>,
    pub message_thread_support: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IntegrationResponse {
    pub id: Uuid,
    pub integration_type: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubOAuthCallback {
    pub code: String,
    pub state: String,
    pub subscription_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SlackOAuthCallback {
    pub code: String,
    pub state: String,
    pub subscription_id: String,
}

/// Setup GitHub integration for a subscription
pub async fn setup_github_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
    Json(req): Json<GitHubIntegrationRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let id = Uuid::new_v4();

    let result = sqlx::query(
        "INSERT INTO github_integrations (
            id, subscription_id, access_token, owner, repository,
            issue_title_template, issue_body_template, auto_create_issues, pr_comment_enabled
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (subscription_id) DO UPDATE SET
            access_token = EXCLUDED.access_token,
            owner = EXCLUDED.owner,
            repository = EXCLUDED.repository,
            issue_title_template = EXCLUDED.issue_title_template,
            issue_body_template = EXCLUDED.issue_body_template,
            auto_create_issues = EXCLUDED.auto_create_issues,
            pr_comment_enabled = EXCLUDED.pr_comment_enabled,
            updated_at = CURRENT_TIMESTAMP"
    )
    .bind(&id)
    .bind(&subscription_id)
    .bind(&req.access_token)
    .bind(&req.owner)
    .bind(&req.repository)
    .bind(&req.issue_title_template)
    .bind(&req.issue_body_template)
    .bind(req.auto_create_issues.unwrap_or(true))
    .bind(req.pr_comment_enabled.unwrap_or(false))
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                subscription_id = %subscription_id,
                owner = %req.owner,
                repository = %req.repository,
                "GitHub integration setup successful"
            );

            Ok((
                StatusCode::CREATED,
                Json(IntegrationResponse {
                    id,
                    integration_type: "github".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                }),
            ))
        }
        Err(e) => {
            error!(
                error = %e,
                subscription_id = %subscription_id,
                "Failed to setup GitHub integration"
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to setup GitHub integration: {}", e),
            ))
        }
    }
}

/// Setup Discord integration for a subscription
pub async fn setup_discord_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
    Json(req): Json<DiscordIntegrationRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let id = Uuid::new_v4();

    let result = sqlx::query(
        "INSERT INTO discord_integrations (
            id, subscription_id, webhook_url, bot_name, avatar_url,
            embed_enabled, thread_support
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (subscription_id) DO UPDATE SET
            webhook_url = EXCLUDED.webhook_url,
            bot_name = EXCLUDED.bot_name,
            avatar_url = EXCLUDED.avatar_url,
            embed_enabled = EXCLUDED.embed_enabled,
            thread_support = EXCLUDED.thread_support,
            updated_at = CURRENT_TIMESTAMP"
    )
    .bind(&id)
    .bind(&subscription_id)
    .bind(&req.webhook_url)
    .bind(&req.bot_name)
    .bind(&req.avatar_url)
    .bind(req.embed_enabled.unwrap_or(true))
    .bind(req.thread_support.unwrap_or(false))
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                subscription_id = %subscription_id,
                webhook = %req.webhook_url,
                "Discord integration setup successful"
            );

            Ok((
                StatusCode::CREATED,
                Json(IntegrationResponse {
                    id,
                    integration_type: "discord".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                }),
            ))
        }
        Err(e) => {
            error!(
                error = %e,
                subscription_id = %subscription_id,
                "Failed to setup Discord integration"
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to setup Discord integration: {}", e),
            ))
        }
    }
}

/// Setup Slack integration for a subscription
pub async fn setup_slack_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
    Json(req): Json<SlackIntegrationRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let id = Uuid::new_v4();

    let result = sqlx::query(
        "INSERT INTO slack_integrations (
            id, subscription_id, webhook_url, bot_token, channel,
            block_kit_enabled, thread_support, user_mentions_enabled
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (subscription_id) DO UPDATE SET
            webhook_url = EXCLUDED.webhook_url,
            bot_token = EXCLUDED.bot_token,
            channel = EXCLUDED.channel,
            block_kit_enabled = EXCLUDED.block_kit_enabled,
            thread_support = EXCLUDED.thread_support,
            user_mentions_enabled = EXCLUDED.user_mentions_enabled,
            updated_at = CURRENT_TIMESTAMP"
    )
    .bind(&id)
    .bind(&subscription_id)
    .bind(&req.webhook_url)
    .bind(&req.bot_token)
    .bind(&req.channel)
    .bind(req.block_kit_enabled.unwrap_or(true))
    .bind(req.thread_support.unwrap_or(false))
    .bind(req.user_mentions_enabled.unwrap_or(false))
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                subscription_id = %subscription_id,
                channel = %req.channel,
                "Slack integration setup successful"
            );

            Ok((
                StatusCode::CREATED,
                Json(IntegrationResponse {
                    id,
                    integration_type: "slack".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                }),
            ))
        }
        Err(e) => {
            error!(
                error = %e,
                subscription_id = %subscription_id,
                "Failed to setup Slack integration"
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to setup Slack integration: {}", e),
            ))
        }
    }
}

/// Setup Telegram integration for a subscription
pub async fn setup_telegram_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
    Json(req): Json<TelegramIntegrationRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let id = Uuid::new_v4();

    let result = sqlx::query(
        "INSERT INTO telegram_integrations (
            id, subscription_id, bot_token, chat_id, webhook_enabled, webhook_url,
            message_thread_support, button_support
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (subscription_id) DO UPDATE SET
            bot_token = EXCLUDED.bot_token,
            chat_id = EXCLUDED.chat_id,
            webhook_enabled = EXCLUDED.webhook_enabled,
            webhook_url = EXCLUDED.webhook_url,
            message_thread_support = EXCLUDED.message_thread_support,
            button_support = EXCLUDED.button_support,
            updated_at = CURRENT_TIMESTAMP"
    )
    .bind(&id)
    .bind(&subscription_id)
    .bind(&req.bot_token)
    .bind(&req.chat_id)
    .bind(req.webhook_enabled.unwrap_or(false))
    .bind(&req.webhook_url)
    .bind(req.message_thread_support.unwrap_or(false))
    .bind(req.button_support.unwrap_or(true))
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            info!(
                subscription_id = %subscription_id,
                chat_id = %req.chat_id,
                "Telegram integration setup successful"
            );

            Ok((
                StatusCode::CREATED,
                Json(IntegrationResponse {
                    id,
                    integration_type: "telegram".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                }),
            ))
        }
        Err(e) => {
            error!(
                error = %e,
                subscription_id = %subscription_id,
                "Failed to setup Telegram integration"
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to setup Telegram integration: {}", e),
            ))
        }
    }
}

/// Get GitHub integration details
pub async fn get_github_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = sqlx::query_as::<_, (Uuid, String, String, String)>(
        "SELECT id, owner, repository, webhook_url FROM github_integrations WHERE subscription_id = $1"
    )
    .bind(&subscription_id)
    .fetch_optional(&pool)
    .await;

    match result {
        Ok(Some((id, owner, repository, _))) => {
            Ok((
                StatusCode::OK,
                Json(json!({
                    "id": id,
                    "integration_type": "github",
                    "owner": owner,
                    "repository": repository,
                })),
            ))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to fetch GitHub integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch integration".to_string(),
            ))
        }
    }
}

/// Get Discord integration details
pub async fn get_discord_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, webhook_url, bot_name FROM discord_integrations WHERE subscription_id = $1"
    )
    .bind(&subscription_id)
    .fetch_optional(&pool)
    .await;

    match result {
        Ok(Some((id, webhook_url, bot_name))) => {
            Ok((
                StatusCode::OK,
                Json(json!({
                    "id": id,
                    "integration_type": "discord",
                    "webhook_url": webhook_url,
                    "bot_name": bot_name,
                })),
            ))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to fetch Discord integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch integration".to_string(),
            ))
        }
    }
}

/// Get Slack integration details
pub async fn get_slack_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, channel FROM slack_integrations WHERE subscription_id = $1"
    )
    .bind(&subscription_id)
    .fetch_optional(&pool)
    .await;

    match result {
        Ok(Some((id, channel))) => {
            Ok((
                StatusCode::OK,
                Json(json!({
                    "id": id,
                    "integration_type": "slack",
                    "channel": channel,
                })),
            ))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to fetch Slack integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch integration".to_string(),
            ))
        }
    }
}

/// Get Telegram integration details
pub async fn get_telegram_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, chat_id FROM telegram_integrations WHERE subscription_id = $1"
    )
    .bind(&subscription_id)
    .fetch_optional(&pool)
    .await;

    match result {
        Ok(Some((id, chat_id))) => {
            Ok((
                StatusCode::OK,
                Json(json!({
                    "id": id,
                    "integration_type": "telegram",
                    "chat_id": chat_id,
                })),
            ))
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to fetch Telegram integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch integration".to_string(),
            ))
        }
    }
}

/// Delete GitHub integration
pub async fn delete_github_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query("DELETE FROM github_integrations WHERE subscription_id = $1")
        .bind(&subscription_id)
        .execute(&pool)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            info!(subscription_id = %subscription_id, "GitHub integration deleted");
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(_) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to delete GitHub integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete integration".to_string(),
            ))
        }
    }
}

/// Delete Discord integration
pub async fn delete_discord_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query("DELETE FROM discord_integrations WHERE subscription_id = $1")
        .bind(&subscription_id)
        .execute(&pool)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            info!(subscription_id = %subscription_id, "Discord integration deleted");
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(_) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to delete Discord integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete integration".to_string(),
            ))
        }
    }
}

/// Delete Slack integration
pub async fn delete_slack_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query("DELETE FROM slack_integrations WHERE subscription_id = $1")
        .bind(&subscription_id)
        .execute(&pool)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            info!(subscription_id = %subscription_id, "Slack integration deleted");
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(_) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to delete Slack integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete integration".to_string(),
            ))
        }
    }
}

/// Delete Telegram integration
pub async fn delete_telegram_integration(
    State(pool): State<PgPool>,
    Path(subscription_id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let result = sqlx::query("DELETE FROM telegram_integrations WHERE subscription_id = $1")
        .bind(&subscription_id)
        .execute(&pool)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            info!(subscription_id = %subscription_id, "Telegram integration deleted");
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(_) => Err((StatusCode::NOT_FOUND, "Integration not found".to_string())),
        Err(e) => {
            error!(error = %e, "Failed to delete Telegram integration");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to delete integration".to_string(),
            ))
        }
    }
}
