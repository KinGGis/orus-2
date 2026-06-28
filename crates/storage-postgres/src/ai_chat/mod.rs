use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::schema::{wf_ai_messages, wf_ai_thread_tags, wf_ai_threads};
use wealthfolio_ai::{
    AiError, ChatMessage, ChatMessageContent, ChatMessagePart, ChatMessageRole,
    ChatRepositoryResult, ChatRepositoryTrait, ChatThread, ChatThreadConfig, ListThreadsRequest,
    ThreadPage, CHAT_MAX_CONTENT_SIZE_BYTES,
};
use wealthfolio_core::{Error as CoreError, errors::DatabaseError};

fn core_to_ai_error(error: CoreError) -> AiError {
    AiError::Core(error)
}

#[derive(Debug, Clone, Queryable, Identifiable, Insertable, AsChangeset, Selectable)]
#[diesel(table_name = wf_ai_threads)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct AiThreadDB {
    id: Uuid,
    title: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    config_snapshot: Option<serde_json::Value>,
    is_pinned: bool,
}

#[derive(Debug, Clone, Queryable, Identifiable, Insertable, AsChangeset, Selectable)]
#[diesel(table_name = wf_ai_messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct AiMessageDB {
    id: Uuid,
    thread_id: Uuid,
    role: String,
    content_json: serde_json::Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Queryable, Identifiable, Insertable, AsChangeset, Selectable)]
#[diesel(table_name = wf_ai_thread_tags)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct AiThreadTagDB {
    id: Uuid,
    thread_id: Uuid,
    tag: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageContent {
    schema_version: u32,
    parts: Vec<MessagePart>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    truncated: bool,
}

impl MessageContent {
    fn from_json(value: &serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value.clone())
    }

    fn to_json_with_limit(&self, max_bytes: usize) -> Result<serde_json::Value, serde_json::Error> {
        let json = serde_json::to_vec(self)?;
        if json.len() <= max_bytes {
            return serde_json::to_value(self);
        }

        let mut truncated_content = self.clone();
        truncated_content.truncated = true;
        truncated_content.truncate_large_payloads(max_bytes);
        serde_json::to_value(truncated_content)
    }

    fn truncate_large_payloads(&mut self, target_bytes: usize) {
        let overhead = 100;
        let available = target_bytes.saturating_sub(overhead);
        let part_count = self.parts.len().max(1);
        let per_part_budget = available / part_count;

        for part in &mut self.parts {
            match part {
                MessagePart::ToolCall { arguments, .. } => {
                    let json = serde_json::to_string(arguments).unwrap_or_default();
                    if json.len() > per_part_budget {
                        *arguments = serde_json::json!({
                            "_truncated": true,
                            "_originalSize": json.len()
                        });
                    }
                }
                MessagePart::ToolResult { data, meta, .. } => {
                    let json = serde_json::to_string(data).unwrap_or_default();
                    if json.len() > per_part_budget {
                        meta.insert("_truncated".to_string(), serde_json::json!(true));
                        meta.insert("_originalSize".to_string(), serde_json::json!(json.len()));
                        *data = serde_json::Value::Null;
                    }
                }
                MessagePart::Text { content }
                | MessagePart::Reasoning { content }
                | MessagePart::System { content } => {
                    if content.len() > per_part_budget {
                        content.truncate(per_part_budget.saturating_sub(20));
                        content.push_str("... [truncated]");
                    }
                }
                MessagePart::Error { message, .. } => {
                    if message.len() > per_part_budget {
                        message.truncate(per_part_budget.saturating_sub(20));
                        message.push_str("... [truncated]");
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum MessagePart {
    System { content: String },
    Text { content: String },
    Reasoning { content: String },
    ToolCall { tool_call_id: String, name: String, arguments: serde_json::Value },
    ToolResult {
        tool_call_id: String,
        success: bool,
        data: serde_json::Value,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        meta: HashMap<String, serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Error { code: String, message: String },
}

fn parse_uuid(value: &str, field: &str) -> ChatRepositoryResult<Uuid> {
    Uuid::parse_str(value).map_err(|err| AiError::InvalidInput(format!("Invalid {field} UUID: {err}")))
}

fn parse_cursor(cursor: &str) -> ChatRepositoryResult<(bool, DateTime<Utc>, Uuid)> {
    let parts: Vec<&str> = cursor.splitn(3, ':').collect();
    if parts.len() != 3 {
        return Err(AiError::InvalidCursor(format!(
            "Expected format 'is_pinned:updated_at:id', got '{cursor}'"
        )));
    }

    let is_pinned = match parts[0] {
        "1" | "true" => true,
        "0" | "false" => false,
        value => {
            return Err(AiError::InvalidCursor(format!(
                "Invalid is_pinned value: {value}"
            )))
        }
    };

    let updated_at = DateTime::parse_from_rfc3339(parts[1])
        .map(|value| value.with_timezone(&Utc))
        .map_err(|err| AiError::InvalidCursor(format!("Invalid updated_at value: {err}")))?;
    let id = parse_uuid(parts[2], "thread_id")?;
    Ok((is_pinned, updated_at, id))
}

fn encode_cursor(is_pinned: bool, updated_at: &DateTime<Utc>, id: &Uuid) -> String {
    format!("{}:{}:{}", if is_pinned { 1 } else { 0 }, updated_at.to_rfc3339(), id)
}

fn thread_to_db(thread: &ChatThread) -> ChatRepositoryResult<AiThreadDB> {
    Ok(AiThreadDB {
        id: parse_uuid(&thread.id, "thread_id")?,
        title: thread.title.clone(),
        created_at: thread.created_at,
        updated_at: thread.updated_at,
        config_snapshot: thread
            .config
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|err| AiError::InvalidInput(err.to_string()))?,
        is_pinned: thread.is_pinned,
    })
}

fn db_to_thread(db: &AiThreadDB) -> ChatThread {
    ChatThread {
        id: db.id.to_string(),
        title: db.title.clone(),
        is_pinned: db.is_pinned,
        tags: Vec::new(),
        config: db
            .config_snapshot
            .as_ref()
            .and_then(|value| serde_json::from_value::<ChatThreadConfig>(value.clone()).ok()),
        created_at: db.created_at,
        updated_at: db.updated_at,
    }
}

fn convert_content_to_json(content: &ChatMessageContent) -> ChatRepositoryResult<serde_json::Value> {
    let storage_parts: Vec<MessagePart> = content
        .parts
        .iter()
        .map(|part| match part {
            ChatMessagePart::System { content } => MessagePart::System { content: content.clone() },
            ChatMessagePart::Text { content } => MessagePart::Text { content: content.clone() },
            ChatMessagePart::Reasoning { content } => MessagePart::Reasoning { content: content.clone() },
            ChatMessagePart::ToolCall { tool_call_id, name, arguments } => MessagePart::ToolCall {
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                arguments: arguments.clone(),
            },
            ChatMessagePart::ToolResult { tool_call_id, success, data, meta, error } => {
                MessagePart::ToolResult {
                    tool_call_id: tool_call_id.clone(),
                    success: *success,
                    data: data.clone(),
                    meta: meta.clone(),
                    error: error.clone(),
                }
            }
            ChatMessagePart::Error { code, message } => MessagePart::Error {
                code: code.clone(),
                message: message.clone(),
            },
        })
        .collect();

    let storage_content = MessageContent {
        schema_version: content.schema_version,
        parts: storage_parts,
        truncated: content.truncated,
    };

    storage_content
        .to_json_with_limit(CHAT_MAX_CONTENT_SIZE_BYTES)
        .map_err(|err| AiError::InvalidInput(err.to_string()))
}

fn convert_json_to_content(value: &serde_json::Value) -> ChatRepositoryResult<ChatMessageContent> {
    let storage_content =
        MessageContent::from_json(value).map_err(|err| AiError::InvalidInput(err.to_string()))?;

    let core_parts = storage_content
        .parts
        .into_iter()
        .map(|part| match part {
            MessagePart::System { content } => ChatMessagePart::System { content },
            MessagePart::Text { content } => ChatMessagePart::Text { content },
            MessagePart::Reasoning { content } => ChatMessagePart::Reasoning { content },
            MessagePart::ToolCall { tool_call_id, name, arguments } => ChatMessagePart::ToolCall {
                tool_call_id,
                name,
                arguments,
            },
            MessagePart::ToolResult { tool_call_id, success, data, meta, error } => {
                ChatMessagePart::ToolResult {
                    tool_call_id,
                    success,
                    data,
                    meta,
                    error,
                }
            }
            MessagePart::Error { code, message } => ChatMessagePart::Error { code, message },
        })
        .collect();

    Ok(ChatMessageContent {
        schema_version: storage_content.schema_version,
        parts: core_parts,
        truncated: storage_content.truncated,
    })
}

fn message_to_db(message: &ChatMessage) -> ChatRepositoryResult<AiMessageDB> {
    Ok(AiMessageDB {
        id: parse_uuid(&message.id, "message_id")?,
        thread_id: parse_uuid(&message.thread_id, "thread_id")?,
        role: message.role.to_string(),
        content_json: convert_content_to_json(&message.content)?,
        created_at: message.created_at,
    })
}

fn db_to_message(db: &AiMessageDB) -> ChatRepositoryResult<ChatMessage> {
    Ok(ChatMessage {
        id: db.id.to_string(),
        thread_id: db.thread_id.to_string(),
        role: db.role.parse::<ChatMessageRole>().map_err(AiError::InvalidInput)?,
        content: convert_json_to_content(&db.content_json)?,
        created_at: db.created_at,
    })
}

pub struct AiChatRepository {
    pool: Arc<DbPool>,
}

impl AiChatRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChatRepositoryTrait for AiChatRepository {
    async fn create_thread(&self, thread: ChatThread) -> ChatRepositoryResult<ChatThread> {
        let thread_db = thread_to_db(&thread)?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let result = diesel::insert_into(wf_ai_threads::table)
            .values(&thread_db)
            .get_result::<AiThreadDB>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        Ok(db_to_thread(&result))
    }

    fn get_thread(&self, thread_id: &str) -> ChatRepositoryResult<Option<ChatThread>> {
        let parsed_id = parse_uuid(thread_id, "thread_id")?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let result = wf_ai_threads::table
            .find(parsed_id)
            .select(AiThreadDB::as_select())
            .first::<AiThreadDB>(&mut conn)
            .optional()
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        Ok(result.map(|db| db_to_thread(&db)))
    }

    fn list_threads(&self, limit: i64, offset: i64) -> ChatRepositoryResult<Vec<ChatThread>> {
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let threads_db = wf_ai_threads::table
            .order((wf_ai_threads::is_pinned.desc(), wf_ai_threads::updated_at.desc(), wf_ai_threads::id.desc()))
            .limit(limit)
            .offset(offset)
            .select(AiThreadDB::as_select())
            .load::<AiThreadDB>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;

        let mut threads = Vec::with_capacity(threads_db.len());
        for db in threads_db {
            let mut thread = db_to_thread(&db);
            thread.tags = wf_ai_thread_tags::table
                .filter(wf_ai_thread_tags::thread_id.eq(db.id))
                .select(wf_ai_thread_tags::tag)
                .load::<String>(&mut conn)
                .unwrap_or_default();
            threads.push(thread);
        }

        Ok(threads)
    }

    fn list_threads_paginated(
        &self,
        request: &ListThreadsRequest,
    ) -> ChatRepositoryResult<ThreadPage> {
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let limit = request.limit.unwrap_or(20).clamp(1, 100) as i64;
        let mut query = wf_ai_threads::table.into_boxed();

        if let Some(search) = &request.search {
            let pattern = format!("%{}%", search);
            query = query.filter(wf_ai_threads::title.ilike(pattern));
        }

        if let Some(cursor) = &request.cursor {
            let (cursor_pinned, cursor_updated_at, cursor_id) = parse_cursor(cursor)?;
            query = query.filter(
                wf_ai_threads::is_pinned
                    .lt(cursor_pinned)
                    .or(wf_ai_threads::is_pinned
                        .eq(cursor_pinned)
                        .and(wf_ai_threads::updated_at.lt(cursor_updated_at)))
                    .or(wf_ai_threads::is_pinned
                        .eq(cursor_pinned)
                        .and(wf_ai_threads::updated_at.eq(cursor_updated_at))
                        .and(wf_ai_threads::id.lt(cursor_id))),
            );
        }

        let threads_db = query
            .order((wf_ai_threads::is_pinned.desc(), wf_ai_threads::updated_at.desc(), wf_ai_threads::id.desc()))
            .select(AiThreadDB::as_select())
            .limit(limit + 1)
            .load::<AiThreadDB>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;

        let has_more = threads_db.len() > limit as usize;
        let threads_db: Vec<_> = threads_db.into_iter().take(limit as usize).collect();
        let mut threads = Vec::with_capacity(threads_db.len());
        for db in &threads_db {
            let mut thread = db_to_thread(db);
            thread.tags = wf_ai_thread_tags::table
                .filter(wf_ai_thread_tags::thread_id.eq(db.id))
                .select(wf_ai_thread_tags::tag)
                .load::<String>(&mut conn)
                .unwrap_or_default();
            threads.push(thread);
        }

        let next_cursor = if has_more {
            threads_db
                .last()
                .map(|thread| encode_cursor(thread.is_pinned, &thread.updated_at, &thread.id))
        } else {
            None
        };

        Ok(ThreadPage {
            threads,
            next_cursor,
            has_more,
        })
    }

    async fn update_thread(&self, thread: ChatThread) -> ChatRepositoryResult<ChatThread> {
        let thread_db = thread_to_db(&thread)?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let result = diesel::update(wf_ai_threads::table.find(thread_db.id))
            .set(&thread_db)
            .get_result::<AiThreadDB>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        Ok(db_to_thread(&result))
    }

    async fn delete_thread(&self, thread_id: &str) -> ChatRepositoryResult<()> {
        let parsed_id = parse_uuid(thread_id, "thread_id")?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        diesel::delete(wf_ai_threads::table.find(parsed_id))
            .execute(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        Ok(())
    }

    async fn create_message(&self, message: ChatMessage) -> ChatRepositoryResult<ChatMessage> {
        let message_db = message_to_db(&message)?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let result = diesel::insert_into(wf_ai_messages::table)
            .values(&message_db)
            .get_result::<AiMessageDB>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;

        diesel::update(wf_ai_threads::table.find(message_db.thread_id))
            .set(wf_ai_threads::updated_at.eq(Utc::now()))
            .execute(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;

        db_to_message(&result)
    }

    fn get_message(&self, message_id: &str) -> ChatRepositoryResult<Option<ChatMessage>> {
        let parsed_id = parse_uuid(message_id, "message_id")?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let result = wf_ai_messages::table
            .find(parsed_id)
            .select(AiMessageDB::as_select())
            .first::<AiMessageDB>(&mut conn)
            .optional()
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        result.map(|db| db_to_message(&db)).transpose()
    }

    fn get_messages_by_thread(&self, thread_id: &str) -> ChatRepositoryResult<Vec<ChatMessage>> {
        let parsed_thread_id = parse_uuid(thread_id, "thread_id")?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let messages_db = wf_ai_messages::table
            .filter(wf_ai_messages::thread_id.eq(parsed_thread_id))
            .order(wf_ai_messages::created_at.asc())
            .select(AiMessageDB::as_select())
            .load::<AiMessageDB>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        messages_db.iter().map(db_to_message).collect()
    }

    async fn update_message(&self, message: ChatMessage) -> ChatRepositoryResult<ChatMessage> {
        let message_db = message_to_db(&message)?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let result = diesel::update(wf_ai_messages::table.find(message_db.id))
            .set(wf_ai_messages::content_json.eq(&message_db.content_json))
            .get_result::<AiMessageDB>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        db_to_message(&result)
    }

    async fn add_tag(&self, thread_id: &str, tag: &str) -> ChatRepositoryResult<()> {
        let parsed_thread_id = parse_uuid(thread_id, "thread_id")?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        let exists = wf_ai_thread_tags::table
            .filter(wf_ai_thread_tags::thread_id.eq(parsed_thread_id))
            .filter(wf_ai_thread_tags::tag.eq(tag))
            .count()
            .get_result::<i64>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;

        if exists == 0 {
            let tag_db = AiThreadTagDB {
                id: Uuid::new_v4(),
                thread_id: parsed_thread_id,
                tag: tag.to_string(),
                created_at: Utc::now(),
            };
            diesel::insert_into(wf_ai_thread_tags::table)
                .values(&tag_db)
                .execute(&mut conn)
                .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        }
        Ok(())
    }

    async fn remove_tag(&self, thread_id: &str, tag: &str) -> ChatRepositoryResult<()> {
        let parsed_thread_id = parse_uuid(thread_id, "thread_id")?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        diesel::delete(
            wf_ai_thread_tags::table
                .filter(wf_ai_thread_tags::thread_id.eq(parsed_thread_id))
                .filter(wf_ai_thread_tags::tag.eq(tag)),
        )
        .execute(&mut conn)
        .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))?;
        Ok(())
    }

    fn get_tags(&self, thread_id: &str) -> ChatRepositoryResult<Vec<String>> {
        let parsed_thread_id = parse_uuid(thread_id, "thread_id")?;
        let mut conn = get_connection(&self.pool).map_err(core_to_ai_error)?;
        wf_ai_thread_tags::table
            .filter(wf_ai_thread_tags::thread_id.eq(parsed_thread_id))
            .select(wf_ai_thread_tags::tag)
            .load::<String>(&mut conn)
            .map_err(|err| AiError::Core(CoreError::Database(DatabaseError::QueryFailed(err.to_string()))))
    }
}