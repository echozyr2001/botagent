use std::{sync::Arc, time::Duration};

use bytebot_agent_rs::database::{DatabaseError, TaskRepositoryTrait};
use bytebot_shared_rs::types::{
    api::{CreateTaskDto, PaginationParams, UpdateTaskDto},
    task::{Role, Task, TaskPriority, TaskStatus, TaskType},
};
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use serde_json::json;
use tokio::runtime::Runtime;
use uuid::Uuid;

// Mock repository for isolated benchmarks
struct MockTaskRepository {
    tasks: std::collections::HashMap<String, Task>,
}

impl MockTaskRepository {
    fn new() -> Self {
        Self {
            tasks: std::collections::HashMap::new(),
        }
    }

    fn with_tasks(mut self, count: usize) -> Self {
        for i in 0..count {
            let task = Task {
                id: Uuid::new_v4().to_string(),
                description: format!("Test task {}", i),
                task_type: TaskType::Immediate,
                status: TaskStatus::Pending,
                priority: TaskPriority::Medium,
                control: Role::Assistant,
                created_at: Utc::now(),
                created_by: Role::User,
                scheduled_for: None,
                updated_at: Utc::now(),
                executed_at: None,
                completed_at: None,
                queued_at: None,
                error: None,
                result: None,
                model: json!({
                    "provider": "anthropic",
                    "name": "claude-3-sonnet-20240229",
                    "title": "Claude 3 Sonnet"
                }),
                user_id: Some("test-user".to_string()),
            };
            self.tasks.insert(task.id.clone(), task);
        }
        self
    }
}

#[async_trait::async_trait]
impl TaskRepositoryTrait for MockTaskRepository {
    async fn create(&self, dto: &CreateTaskDto) -> Result<Task, DatabaseError> {
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
        Ok(task)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Task>, DatabaseError> {
        Ok(self.tasks.get(id).cloned())
    }

    async fn update(&self, _id: &str, _dto: &UpdateTaskDto) -> Result<Option<Task>, DatabaseError> {
        // Simplified mock implementation
        Ok(None)
    }

    async fn delete(&self, _id: &str) -> Result<bool, DatabaseError> {
        Ok(true)
    }

    async fn list(
        &self,
        _filter: &bytebot_agent_rs::database::TaskFilter,
        pagination: &PaginationParams,
    ) -> Result<(Vec<Task>, u64), DatabaseError> {
        let limit = pagination.limit.unwrap_or(20) as usize;
        let tasks: Vec<Task> = self.tasks.values().take(limit).cloned().collect();
        Ok((tasks, self.tasks.len() as u64))
    }

    async fn update_status(
        &self,
        _id: &str,
        _status: TaskStatus,
    ) -> Result<Option<Task>, DatabaseError> {
        Ok(None)
    }

    async fn get_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, DatabaseError> {
        let tasks: Vec<Task> = self
            .tasks
            .values()
            .filter(|t| t.status == status)
            .cloned()
            .collect();
        Ok(tasks)
    }

    async fn get_scheduled_tasks(
        &self,
        _before: chrono::DateTime<Utc>,
    ) -> Result<Vec<Task>, DatabaseError> {
        let tasks: Vec<Task> = self
            .tasks
            .values()
            .filter(|t| t.task_type == TaskType::Scheduled)
            .cloned()
            .collect();
        Ok(tasks)
    }

    async fn count_by_status(
        &self,
    ) -> Result<std::collections::HashMap<TaskStatus, u64>, DatabaseError> {
        let mut counts = std::collections::HashMap::new();
        for task in self.tasks.values() {
            *counts.entry(task.status).or_insert(0) += 1;
        }
        Ok(counts)
    }
}

// Helper functions for creating test data
fn create_test_task_dto() -> CreateTaskDto {
    CreateTaskDto {
        description: "Test task for benchmarking".to_string(),
        task_type: Some(TaskType::Immediate),
        priority: Some(TaskPriority::Medium),
        created_by: Some(Role::User),
        scheduled_for: None,
        model: Some(json!({
            "provider": "anthropic",
            "name": "claude-3-sonnet-20240229",
            "title": "Claude 3 Sonnet"
        })),
        user_id: Some("test-user".to_string()),
        files: None,
    }
}

fn create_test_update_dto() -> UpdateTaskDto {
    UpdateTaskDto {
        status: Some(TaskStatus::Running),
        priority: Some(TaskPriority::High),
        queued_at: Some(Utc::now()),
        executed_at: Some(Utc::now()),
        completed_at: None,
    }
}

// Task repository benchmarks
fn benchmark_task_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let repo = MockTaskRepository::new();

    c.bench_function("task_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let dto = create_test_task_dto();
                black_box(repo.create(&dto).await.unwrap())
            })
        })
    });
}

fn benchmark_task_retrieval(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let repo = MockTaskRepository::new().with_tasks(1000);
    let task_ids: Vec<String> = repo.tasks.keys().cloned().collect();

    c.bench_function("task_retrieval", |b| {
        b.iter_batched(
            || task_ids[fastrand::usize(..task_ids.len())].clone(),
            |task_id| rt.block_on(async { black_box(repo.get_by_id(&task_id).await.unwrap()) }),
            BatchSize::SmallInput,
        )
    });
}

fn benchmark_task_listing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("task_listing");

    for size in [10, 100, 1000, 10000].iter() {
        let repo = MockTaskRepository::new().with_tasks(*size);
        let filter = bytebot_agent_rs::database::TaskFilter::default();
        let pagination = PaginationParams {
            page: Some(1),
            limit: Some(20),
        };

        group.bench_with_input(BenchmarkId::new("list_tasks", size), size, |b, _| {
            b.iter(|| {
                rt.block_on(async { black_box(repo.list(&filter, &pagination).await.unwrap()) })
            })
        });
    }

    group.finish();
}

fn benchmark_concurrent_task_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let repo = Arc::new(MockTaskRepository::new().with_tasks(1000));

    let mut group = c.benchmark_group("concurrent_operations");

    for concurrency in [1, 5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_creates", concurrency),
            concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    rt.block_on(async {
                        let repo = repo.clone();
                        let handles: Vec<_> = (0..concurrency)
                            .map(|_| {
                                let repo = repo.clone();
                                tokio::spawn(async move {
                                    let dto = create_test_task_dto();
                                    repo.create(&dto).await.unwrap()
                                })
                            })
                            .collect();

                        let results: Vec<_> = futures::future::join_all(handles)
                            .await
                            .into_iter()
                            .map(|r| r.unwrap())
                            .collect();

                        black_box(results)
                    })
                })
            },
        );
    }

    group.finish();
}

// Task serialization benchmarks
fn benchmark_task_serialization(c: &mut Criterion) {
    let task = Task {
        id: Uuid::new_v4().to_string(),
        description: "Test task for serialization benchmarking".to_string(),
        task_type: TaskType::Immediate,
        status: TaskStatus::Pending,
        priority: TaskPriority::Medium,
        control: Role::Assistant,
        created_at: Utc::now(),
        created_by: Role::User,
        scheduled_for: None,
        updated_at: Utc::now(),
        executed_at: None,
        completed_at: None,
        queued_at: None,
        error: None,
        result: Some(json!({"result": "test data", "metrics": {"duration": 1000}})),
        model: json!({
            "provider": "anthropic",
            "name": "claude-3-sonnet-20240229",
            "title": "Claude 3 Sonnet"
        }),
        user_id: Some("test-user".to_string()),
    };

    c.bench_function("task_serialization", |b| {
        b.iter(|| black_box(serde_json::to_string(&task).unwrap()))
    });

    let serialized = serde_json::to_string(&task).unwrap();
    c.bench_function("task_deserialization", |b| {
        b.iter(|| black_box(serde_json::from_str::<Task>(&serialized).unwrap()))
    });
}

// Task validation benchmarks
fn benchmark_task_validation(c: &mut Criterion) {
    let task = Task {
        id: Uuid::new_v4().to_string(),
        description: "Test task for validation benchmarking".to_string(),
        task_type: TaskType::Immediate,
        status: TaskStatus::Pending,
        priority: TaskPriority::Medium,
        control: Role::Assistant,
        created_at: Utc::now(),
        created_by: Role::User,
        scheduled_for: None,
        updated_at: Utc::now(),
        executed_at: None,
        completed_at: None,
        queued_at: None,
        error: None,
        result: None,
        model: json!({
            "provider": "anthropic",
            "name": "claude-3-sonnet-20240229",
            "title": "Claude 3 Sonnet"
        }),
        user_id: Some("test-user".to_string()),
    };

    c.bench_function("task_validation", |b| {
        b.iter(|| black_box(task.validate_integrity().unwrap()))
    });
}

// Memory usage benchmarks
fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    for count in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("task_creation_memory", count),
            count,
            |b, &count| {
                b.iter(|| {
                    let tasks: Vec<Task> = (0..count)
                        .map(|i| Task {
                            id: Uuid::new_v4().to_string(),
                            description: format!("Test task {}", i),
                            task_type: TaskType::Immediate,
                            status: TaskStatus::Pending,
                            priority: TaskPriority::Medium,
                            control: Role::Assistant,
                            created_at: Utc::now(),
                            created_by: Role::User,
                            scheduled_for: None,
                            updated_at: Utc::now(),
                            executed_at: None,
                            completed_at: None,
                            queued_at: None,
                            error: None,
                            result: None,
                            model: json!({
                                "provider": "anthropic",
                                "name": "claude-3-sonnet-20240229",
                                "title": "Claude 3 Sonnet"
                            }),
                            user_id: Some("test-user".to_string()),
                        })
                        .collect();

                    black_box(tasks)
                })
            },
        );
    }

    group.finish();
}

// Load testing simulation
fn benchmark_load_testing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let repo = Arc::new(MockTaskRepository::new().with_tasks(10000));

    let mut group = c.benchmark_group("load_testing");
    group.measurement_time(Duration::from_secs(10));

    // Simulate mixed workload
    group.bench_function("mixed_workload", |b| {
        b.iter(|| {
            rt.block_on(async {
                let repo = repo.clone();

                // Simulate 70% reads, 20% creates, 10% updates
                let operations = 100;
                let mut handles = Vec::new();

                for i in 0..operations {
                    let repo = repo.clone();
                    let handle = match i % 10 {
                        0..=6 => {
                            // Read operations (70%)
                            tokio::spawn(async move {
                                let filter = bytebot_agent_rs::database::TaskFilter::default();
                                let pagination = PaginationParams {
                                    page: Some(1),
                                    limit: Some(10),
                                };
                                repo.list(&filter, &pagination).await.unwrap();
                                "read"
                            })
                        }
                        7..=8 => {
                            // Create operations (20%)
                            tokio::spawn(async move {
                                let dto = create_test_task_dto();
                                repo.create(&dto).await.unwrap();
                                "create"
                            })
                        }
                        _ => {
                            // Update operations (10%)
                            tokio::spawn(async move {
                                let dto = create_test_update_dto();
                                repo.update("test-id", &dto).await.unwrap_or_default();
                                "update"
                            })
                        }
                    };
                    handles.push(handle);
                }

                let _results = futures::future::join_all(handles).await;
                black_box(())
            })
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_task_creation,
    benchmark_task_retrieval,
    benchmark_task_listing,
    benchmark_concurrent_task_operations,
    benchmark_task_serialization,
    benchmark_task_validation,
    benchmark_memory_usage,
    benchmark_load_testing
);
criterion_main!(benches);
