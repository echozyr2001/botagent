use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::task::Role;

/// Message content type enum for Anthropic content blocks
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageContentType {
    Text,
    Image,
    Document,
    ToolUse,
    ToolResult,
    Thinking,
    RedactedThinking,
}

/// Base content block structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockBase {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<MessageContentBlock>>,
}

/// Text content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContentBlock {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    pub text: String,
}

/// Image source structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    pub media_type: String, // e.g., "image/png"
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub data: String,       // Base64 encoded image data
}

/// Image content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContentBlock {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    pub source: ImageSource,
}

/// Document source structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String,
    pub data: String, // Base64 encoded document data
}

/// Document content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContentBlock {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    pub source: DocumentSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
}

/// Tool use content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseContentBlock {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    pub name: String,
    pub id: String,
    pub input: serde_json::Value,
}

/// Tool result content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultContentBlock {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    pub tool_use_id: String,
    pub content: Vec<MessageContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Thinking content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingContentBlock {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    pub thinking: String,
    pub signature: String,
}

/// Redacted thinking content block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedThinkingContentBlock {
    #[serde(rename = "type")]
    pub content_type: MessageContentType,
    pub data: String,
}

/// Union type for all message content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContentBlock {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    Document {
        source: DocumentSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        size: Option<i64>,
    },
    ToolUse {
        name: String,
        id: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<MessageContentBlock>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

impl MessageContentBlock {
    /// Create a text content block
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create an image content block
    pub fn image(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource {
                media_type: media_type.into(),
                source_type: "base64".to_string(),
                data: data.into(),
            },
        }
    }

    /// Create a tool use content block
    pub fn tool_use(
        name: impl Into<String>,
        id: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self::ToolUse {
            name: name.into(),
            id: id.into(),
            input,
        }
    }

    /// Create a tool result content block
    pub fn tool_result(tool_use_id: impl Into<String>, content: Vec<MessageContentBlock>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content,
            is_error: None,
        }
    }

    /// Get the content type of this block
    pub fn content_type(&self) -> MessageContentType {
        match self {
            Self::Text { .. } => MessageContentType::Text,
            Self::Image { .. } => MessageContentType::Image,
            Self::Document { .. } => MessageContentType::Document,
            Self::ToolUse { .. } => MessageContentType::ToolUse,
            Self::ToolResult { .. } => MessageContentType::ToolResult,
            Self::Thinking { .. } => MessageContentType::Thinking,
            Self::RedactedThinking { .. } => MessageContentType::RedactedThinking,
        }
    }

    /// Extract text content if this is a text block
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }

    /// Check if this is an error tool result
    pub fn is_error_result(&self) -> bool {
        match self {
            Self::ToolResult { is_error, .. } => is_error.unwrap_or(false),
            _ => false,
        }
    }
}

/// Message entity matching Prisma schema
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct Message {
    pub id: String,

    /// Content field follows Anthropic's content blocks structure
    /// Example: [{"type": "text", "text": "Hello world"}, {"type": "image", "source": {...}}]
    pub content: serde_json::Value,

    pub role: Role,

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,

    #[serde(rename = "taskId")]
    pub task_id: String,

    #[serde(rename = "summaryId")]
    pub summary_id: Option<String>,

    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

impl Message {
    /// Create a new message with default values
    pub fn new(content: Vec<MessageContentBlock>, role: Role, task_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            content: serde_json::to_value(content).unwrap_or(serde_json::Value::Array(vec![])),
            role,
            created_at: now,
            updated_at: now,
            task_id,
            summary_id: None,
            user_id: None,
        }
    }

    /// Get content blocks as typed structures
    pub fn get_content_blocks(&self) -> Result<Vec<MessageContentBlock>, serde_json::Error> {
        serde_json::from_value(self.content.clone())
    }

    /// Set content blocks from typed structures
    pub fn set_content_blocks(
        &mut self,
        blocks: Vec<MessageContentBlock>,
    ) -> Result<(), serde_json::Error> {
        self.content = serde_json::to_value(blocks)?;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Extract all text content from the message
    pub fn extract_text(&self) -> String {
        if let Ok(blocks) = self.get_content_blocks() {
            blocks
                .iter()
                .filter_map(|block| block.as_text())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            String::new()
        }
    }

    /// Check if message contains any tool use blocks
    pub fn has_tool_use(&self) -> bool {
        if let Ok(blocks) = self.get_content_blocks() {
            blocks
                .iter()
                .any(|block| matches!(block, MessageContentBlock::ToolUse { .. }))
        } else {
            false
        }
    }

    /// Check if message contains any error tool results
    pub fn has_error_results(&self) -> bool {
        if let Ok(blocks) = self.get_content_blocks() {
            blocks.iter().any(|block| block.is_error_result())
        } else {
            false
        }
    }

    /// Validate message content structure
    pub fn validate_content(&self) -> Result<(), String> {
        // Ensure content is an array
        if !self.content.is_array() {
            return Err("Message content must be an array of content blocks".to_string());
        }

        // Try to deserialize content blocks to validate structure
        match self.get_content_blocks() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Invalid content block structure: {e}")),
        }
    }
}

/// Summary entity matching Prisma schema
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct Summary {
    pub id: String,

    #[validate(length(
        min = 1,
        max = 50000,
        message = "Summary content must be between 1 and 50000 characters"
    ))]
    pub content: String,

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,

    #[serde(rename = "taskId")]
    pub task_id: String,

    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
}

impl Summary {
    /// Create a new summary with default values
    pub fn new(content: String, task_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            content,
            created_at: now,
            updated_at: now,
            task_id,
            parent_id: None,
        }
    }

    /// Create a child summary
    pub fn new_child(content: String, task_id: String, parent_id: String) -> Self {
        let mut summary = Self::new(content, task_id);
        summary.parent_id = Some(parent_id);
        summary
    }
}
