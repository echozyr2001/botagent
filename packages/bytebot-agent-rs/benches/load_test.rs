use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use bytebot_agent_rs::database::{DatabaseError, TaskRepositoryTrait};
use bytebot_shared_rs::types::{
    api::{CreateTaskDto, PaginationParams},
    task::{Role, Task, TaskPriority, TaskStatus, TaskType},
};
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use tokio::runtime::Runtime;
use uuid::Uuid;

// Mock repository for load testing
struct LoadTestRepository {
    tasks: std::sync::RwLock<std::collections::HashMap<String, Task>>,
    operation_count: std::sync::atomic::AtomicUsize,
}

impl LoadTestRepository {
    fn new() -> Self {
        Self {
            tasks: std::sync::RwLock::new(std::collections::HashMap::new()),
            operation_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn get_operation_count(&self) -> usize {
        self.operation_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn reset_operation_count(&self) {
        self.operation_count
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

#[async_trait::async_trait]
impl TaskRepositoryTrait for LoadTestRepository {
    async fn create(&self, dto: &CreateTaskDto) -> Result<Task, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(1)).await;

        let task = Task {
            id: Uuid::new_v4().to_string(),
            description: dto.description.clone(),
            task_type: dto.task_type.unwrap_or_default(),
            status: TaskStatus::Pending,
            priority: dto.priority.unwrap_or_default(),
            control: Role::Assistant,
            created_at: Utc::now(),
            created_by: dto.created_by.unwrap_or(Role::User),
            scheduled_for: dto.scheduled_for,
            updated_at: Utc::now(),
            executed_at: None,
            completed_at: None,
            queued_at: None,
            error: None,
            result: None,
            model: dto.model.clone().unwrap_or_else(|| {
                json!({
                    "provider": "anthropic",
                    "name": "claude-3-sonnet-20240229",
                    "title": "Claude 3 Sonnet"
                })
            }),
            user_id: dto.user_id.clone(),
        };

        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.insert(task.id.clone(), task.clone());
        }

        Ok(task)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Task>, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(1)).await;

        let tasks = self.tasks.read().unwrap();
        Ok(tasks.get(id).cloned())
    }

    async fn update(
        &self,
        id: &str,
        _dto: &bytebot_shared_rs::types::api::UpdateTaskDto,
    ) -> Result<Option<Task>, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(2)).await;

        let tasks = self.tasks.read().unwrap();
        Ok(tasks.get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<bool, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(1)).await;

        let mut tasks = self.tasks.write().unwrap();
        Ok(tasks.remove(id).is_some())
    }

    async fn list(
        &self,
        _filter: &bytebot_agent_rs::database::TaskFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Task>, u64), DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(3)).await;

        let tasks = self.tasks.read().unwrap();
        let limit = pagination.limit.unwrap_or(20) as usize;
        let task_list: Vec<_> = tasks.values().take(limit).cloned().collect();
        Ok((task_list, tasks.len() as u64))
    }

    async fn update_status(
        &self,
        id: &str,
        _status: TaskStatus,
    ) -> Result<Option<Task>, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(2)).await;

        let tasks = self.tasks.read().unwrap();
        Ok(tasks.get(id).cloned())
    }

    async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(5)).await;

        let tasks = self.tasks.read().unwrap();
        let filtered: Vec<_> = tasks
            .values()
            .filter(|t| t.status == status)
            .cloned()
            .collect();
        Ok(filtered)
    }

    async fn get_scheduled_tasks(
        &self,
        _before: chrono::DateTime<Utc>,
    ) -> Result<Vec<Task>, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(5)).await;

        let tasks = self.tasks.read().unwrap();
        let scheduled: Vec<_> = tasks
            .values()
            .filter(|t| t.task_type == TaskType::Scheduled)
            .cloned()
            .collect();
        Ok(scheduled)
    }

    async fn count_by_status(
        &self,
    ) -> Result<std::collections::HashMap<TaskStatus, u64>, DatabaseError> {
        self.operation_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Simulate database latency
        tokio::time::sleep(Duration::from_millis(10)).await;

        let tasks = self.tasks.read().unwrap();
        let mut counts = std::collections::HashMap::new();
        for task in tasks.values() {
            *counts.entry(task.status).or_insert(0) += 1;
        }
        Ok(counts)
    }
}

// Helper function to create test DTOs
fn create_load_test_dto(i: usize) -> CreateTaskDto {
    CreateTaskDto {
        description: format!("Load test task {}", i),
        task_type: Some(if i % 3 == 0 {
            TaskType::Scheduled
        } else {
            TaskType::Immediate
        }),
        priority: Some(match i % 4 {
            0 => TaskPriority::Low,
            1 => TaskPriority::Medium,
            2 => TaskPriority::High,
            _ => TaskPriority::Urgent,
        }),
        created_by: Some(Role::User),
        scheduled_for: if i % 3 == 0 {
            Some(Utc::now() + chrono::Duration::hours(1))
        } else {
            None
        },
        model: Some(json!({
            "provider": "anthropic",
            "name": "claude-3-sonnet-20240229",
            "title": "Claude 3 Sonnet"
        })),
        user_id: Some(format!("user-{}", i % 10)),
        files: None,
    }
}

// Simple load test
fn benchmark_simple_load_test(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let repo = Arc::new(LoadTestRepository::new());

    c.bench_function("simple_load_test", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = repo.clone();
                repo.reset_operation_count();

                // Create some tasks
                for i in 0..10 {
                    let dto = create_load_test_dto(i);
                    let _ = repo.create(&dto).await;
                }

                // List tasks
                let filter = bytebot_agent_rs::database::TaskFilter::default();
                let pagination = PaginationParams {
                    page: Some(1),
                    limit: Some(10),
                };
                let (tasks, total) = repo.list(&filter, &pagination).await.unwrap();

                black_box((tasks.len(), total))
            })
        })
    });
}

criterion_group!(load_tests, benchmark_simple_load_test);
criterion_main!(load_tests);
