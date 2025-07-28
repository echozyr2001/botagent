use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;
use validator::Validate;

/// Task status enum matching Prisma schema
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[sqlx(type_name = "TaskStatus", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskStatus {
    Pending,
    Running,
    NeedsHelp,
    NeedsReview,
    Completed,
    Cancelled,
    Failed,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "RUNNING" => Ok(Self::Running),
            "NEEDSHELP" => Ok(Self::NeedsHelp),
            "NEEDSREVIEW" => Ok(Self::NeedsReview),
            "COMPLETED" => Ok(Self::Completed),
            "CANCELLED" => Ok(Self::Cancelled),
            "FAILED" => Ok(Self::Failed),
            _ => Err(format!("Invalid TaskStatus: {s}")),
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::Running => write!(f, "RUNNING"),
            Self::NeedsHelp => write!(f, "NEEDSHELP"),
            Self::NeedsReview => write!(f, "NEEDSREVIEW"),
            Self::Completed => write!(f, "COMPLETED"),
            Self::Cancelled => write!(f, "CANCELLED"),
            Self::Failed => write!(f, "FAILED"),
        }
    }
}

/// Task priority enum matching Prisma schema
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "TaskPriority", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Medium
    }
}

impl std::str::FromStr for TaskPriority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LOW" => Ok(Self::Low),
            "MEDIUM" => Ok(Self::Medium),
            "HIGH" => Ok(Self::High),
            "URGENT" => Ok(Self::Urgent),
            _ => Err(format!("Invalid TaskPriority: {s}")),
        }
    }
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Urgent => write!(f, "URGENT"),
        }
    }
}

/// Role enum matching Prisma schema
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "Role", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum Role {
    User,
    Assistant,
}

impl Default for Role {
    fn default() -> Self {
        Self::Assistant
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "USER" => Ok(Self::User),
            "ASSISTANT" => Ok(Self::Assistant),
            _ => Err(format!("Invalid Role: {s}")),
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "USER"),
            Self::Assistant => write!(f, "ASSISTANT"),
        }
    }
}

/// Task type enum matching Prisma schema
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "TaskType", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskType {
    Immediate,
    Scheduled,
}

impl Default for TaskType {
    fn default() -> Self {
        Self::Immediate
    }
}

impl std::str::FromStr for TaskType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "IMMEDIATE" => Ok(Self::Immediate),
            "SCHEDULED" => Ok(Self::Scheduled),
            _ => Err(format!("Invalid TaskType: {s}")),
        }
    }
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Immediate => write!(f, "IMMEDIATE"),
            Self::Scheduled => write!(f, "SCHEDULED"),
        }
    }
}

/// Task entity matching Prisma schema
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Validate)]
pub struct Task {
    pub id: String,

    #[validate(length(
        min = 1,
        max = 10000,
        message = "Description must be between 1 and 10000 characters"
    ))]
    pub description: String,

    #[serde(rename = "type")]
    pub task_type: TaskType,

    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub control: Role,

    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,

    #[serde(rename = "createdBy")]
    pub created_by: Role,

    #[serde(rename = "scheduledFor")]
    pub scheduled_for: Option<DateTime<Utc>>,

    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,

    #[serde(rename = "executedAt")]
    pub executed_at: Option<DateTime<Utc>>,

    #[serde(rename = "completedAt")]
    pub completed_at: Option<DateTime<Utc>>,

    #[serde(rename = "queuedAt")]
    pub queued_at: Option<DateTime<Utc>>,

    pub error: Option<String>,
    pub result: Option<serde_json::Value>,

    /// Model configuration as JSON
    /// Example: {"provider": "anthropic", "name": "claude-opus-4-20250514", "title": "Claude Opus 4"}
    pub model: serde_json::Value,

    #[serde(rename = "userId")]
    pub user_id: Option<String>,
}

impl Task {
    /// Create a new task with default values
    pub fn new(description: String, model: serde_json::Value) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            description,
            task_type: TaskType::default(),
            status: TaskStatus::default(),
            priority: TaskPriority::default(),
            control: Role::default(),
            created_at: now,
            created_by: Role::User,
            scheduled_for: None,
            updated_at: now,
            executed_at: None,
            completed_at: None,
            queued_at: None,
            error: None,
            result: None,
            model,
            user_id: None,
        }
    }

    /// Check if task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Failed
        )
    }

    /// Check if task is active (running or needs attention)
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Running | TaskStatus::NeedsHelp | TaskStatus::NeedsReview
        )
    }

    /// Validate task data integrity
    pub fn validate_integrity(&self) -> Result<(), String> {
        // Validate that completed tasks have completion timestamp
        if self.status == TaskStatus::Completed && self.completed_at.is_none() {
            return Err("Completed tasks must have completion timestamp".to_string());
        }

        // Validate that scheduled tasks have scheduled_for timestamp
        if self.task_type == TaskType::Scheduled && self.scheduled_for.is_none() {
            return Err("Scheduled tasks must have scheduled_for timestamp".to_string());
        }

        // Validate that executed tasks have execution timestamp
        if matches!(
            self.status,
            TaskStatus::Running | TaskStatus::Completed | TaskStatus::Failed
        ) && self.executed_at.is_none()
        {
            return Err("Executed tasks must have execution timestamp".to_string());
        }

        // Validate model structure
        if !self.model.is_object() {
            return Err("Model must be a JSON object".to_string());
        }

        let model_obj = self.model.as_object().unwrap();
        if !model_obj.contains_key("provider") || !model_obj.contains_key("name") {
            return Err("Model must contain 'provider' and 'name' fields".to_string());
        }

        Ok(())
    }
}
