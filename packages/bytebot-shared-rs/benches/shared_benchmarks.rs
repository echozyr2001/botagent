use std::time::Duration;

use bytebot_shared_rs::types::{
    api::CreateTaskDto,
    computer_action::{Button, ComputerAction, Coordinates},
    message::{Message, MessageContentBlock},
    task::{Role, Task, TaskPriority, TaskStatus, TaskType},
    user::User,
};
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use uuid::Uuid;

// Helper functions for creating test data
fn create_test_task() -> Task {
    Task {
        id: Uuid::new_v4().to_string(),
        description: "Test task for benchmarking shared types".to_string(),
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
        result: Some(json!({
            "output": "Task completed successfully",
            "metrics": {
                "duration": 1500,
                "memory_used": 1024,
                "cpu_usage": 0.75
            }
        })),
        model: json!({
            "provider": "anthropic",
            "name": "claude-3-sonnet-20240229",
            "title": "Claude 3 Sonnet"
        }),
        user_id: Some("test-user-id".to_string()),
    }
}

fn create_test_message() -> Message {
    Message::new(
        vec![
            MessageContentBlock::text("This is a test message for benchmarking"),
            MessageContentBlock::text("It contains multiple content blocks"),
        ],
        Role::User,
        "test-task-id".to_string(),
    )
}

fn create_test_user() -> User {
    User {
        id: Uuid::new_v4().to_string(),
        email: "test@example.com".to_string(),
        name: Some("Test User".to_string()),
        image: Some("https://example.com/avatar.jpg".to_string()),
        email_verified: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_test_computer_action() -> ComputerAction {
    ComputerAction::DragMouse {
        path: vec![
            Coordinates { x: 100, y: 100 },
            Coordinates { x: 200, y: 200 },
            Coordinates { x: 300, y: 300 },
            Coordinates { x: 400, y: 400 },
        ],
        button: Button::Left,
        hold_keys: Some(vec!["shift".to_string(), "ctrl".to_string()]),
    }
}

fn create_test_create_task_dto() -> CreateTaskDto {
    CreateTaskDto {
        description: "Test task creation DTO for benchmarking".to_string(),
        task_type: Some(TaskType::Scheduled),
        priority: Some(TaskPriority::High),
        created_by: Some(Role::User),
        scheduled_for: Some(Utc::now() + chrono::Duration::hours(1)),
        model: Some(json!({
            "provider": "openai",
            "name": "gpt-4o",
            "title": "GPT-4o"
        })),
        user_id: Some("test-user-id".to_string()),
        files: None,
    }
}

// Type serialization benchmarks
fn benchmark_type_serialization(c: &mut Criterion) {
    let task = create_test_task();
    let message = create_test_message();
    let _user = create_test_user();
    let action = create_test_computer_action();
    let dto = create_test_create_task_dto();

    let mut group = c.benchmark_group("type_serialization");

    // Task serialization
    group.bench_function("task_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&task).unwrap()))
    });

    let task_json = serde_json::to_string(&task).unwrap();
    group.bench_function("task_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<Task>(&task_json).unwrap()))
    });

    // Message serialization
    group.bench_function("message_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&message).unwrap()))
    });

    let message_json = serde_json::to_string(&message).unwrap();
    group.bench_function("message_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<Message>(&message_json).unwrap()))
    });

    // Computer action serialization
    group.bench_function("computer_action_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&action).unwrap()))
    });

    let action_json = serde_json::to_string(&action).unwrap();
    group.bench_function("computer_action_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<ComputerAction>(&action_json).unwrap()))
    });

    // DTO serialization
    group.bench_function("dto_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&dto).unwrap()))
    });

    let dto_json = serde_json::to_string(&dto).unwrap();
    group.bench_function("dto_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<CreateTaskDto>(&dto_json).unwrap()))
    });

    group.finish();
}

// Type validation benchmarks
fn benchmark_type_validation(c: &mut Criterion) {
    let task = create_test_task();

    let mut group = c.benchmark_group("type_validation");

    group.bench_function("task_validation", |b| {
        b.iter(|| black_box(task.validate_integrity().unwrap()))
    });

    group.bench_function("task_is_terminal", |b| {
        b.iter(|| black_box(task.is_terminal()))
    });

    group.bench_function("task_is_active", |b| b.iter(|| black_box(task.is_active())));

    group.finish();
}

// Enum conversion benchmarks
fn benchmark_enum_conversions(c: &mut Criterion) {
    let mut group = c.benchmark_group("enum_conversions");

    // TaskStatus conversions
    let status_strings = vec!["PENDING", "RUNNING", "COMPLETED", "FAILED"];
    group.bench_function("task_status_from_str", |b| {
        b.iter(|| {
            let status_str = status_strings[fastrand::usize(..status_strings.len())];
            black_box(status_str.parse::<TaskStatus>().unwrap())
        })
    });

    let statuses = vec![
        TaskStatus::Pending,
        TaskStatus::Running,
        TaskStatus::Completed,
        TaskStatus::Failed,
    ];
    group.bench_function("task_status_to_string", |b| {
        b.iter(|| {
            let status = statuses[fastrand::usize(..statuses.len())];
            black_box(status.to_string())
        })
    });

    // TaskPriority conversions
    let priority_strings = vec!["LOW", "MEDIUM", "HIGH", "URGENT"];
    group.bench_function("task_priority_from_str", |b| {
        b.iter(|| {
            let priority_str = priority_strings[fastrand::usize(..priority_strings.len())];
            black_box(priority_str.parse::<TaskPriority>().unwrap())
        })
    });

    // Role conversions
    let role_strings = vec!["USER", "ASSISTANT"];
    group.bench_function("role_from_str", |b| {
        b.iter(|| {
            let role_str = role_strings[fastrand::usize(..role_strings.len())];
            black_box(role_str.parse::<Role>().unwrap())
        })
    });

    group.finish();
}

// Collection operations benchmarks
fn benchmark_collection_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("collection_operations");

    for size in [10, 100, 1000, 10000].iter() {
        // Task collection creation
        group.bench_with_input(
            BenchmarkId::new("task_collection_creation", size),
            size,
            |b, size| {
                b.iter(|| {
                    let tasks: Vec<Task> = (0..*size)
                        .map(|i| {
                            let mut task = create_test_task();
                            task.description = format!("Task {}", i);
                            task
                        })
                        .collect();
                    black_box(tasks)
                })
            },
        );

        // Task collection serialization
        let tasks: Vec<Task> = (0..*size)
            .map(|i| {
                let mut task = create_test_task();
                task.description = format!("Task {}", i);
                task
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("task_collection_serialize", size),
            size,
            |b, _| b.iter(|| black_box(serde_json::to_string(&tasks).unwrap())),
        );

        // Message collection creation
        group.bench_with_input(
            BenchmarkId::new("message_collection_creation", size),
            size,
            |b, size| {
                b.iter(|| {
                    let messages: Vec<Message> = (0..*size)
                        .map(|i| {
                            Message::new(
                                vec![MessageContentBlock::text(&format!("Message {}", i))],
                                if i % 2 == 0 {
                                    Role::User
                                } else {
                                    Role::Assistant
                                },
                                format!("task-{}", i),
                            )
                        })
                        .collect();
                    black_box(messages)
                })
            },
        );
    }

    group.finish();
}

// Memory usage benchmarks
fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    for count in [100, 1000, 10000].iter() {
        // Task memory usage
        group.bench_with_input(
            BenchmarkId::new("task_memory_allocation", count),
            count,
            |b, count| {
                b.iter(|| {
                    let tasks: Vec<Task> = (0..*count).map(|_| create_test_task()).collect();
                    black_box(tasks)
                })
            },
        );

        // Message memory usage
        group.bench_with_input(
            BenchmarkId::new("message_memory_allocation", count),
            count,
            |b, count| {
                b.iter(|| {
                    let messages: Vec<Message> =
                        (0..*count).map(|_| create_test_message()).collect();
                    black_box(messages)
                })
            },
        );

        // Computer action memory usage
        group.bench_with_input(
            BenchmarkId::new("computer_action_memory_allocation", count),
            count,
            |b, count| {
                b.iter(|| {
                    let actions: Vec<ComputerAction> = (0..*count)
                        .map(|i| ComputerAction::MoveMouse {
                            coordinates: Coordinates {
                                x: i as i32,
                                y: i as i32,
                            },
                        })
                        .collect();
                    black_box(actions)
                })
            },
        );
    }

    group.finish();
}

// Complex data structure benchmarks
fn benchmark_complex_data_structures(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_data_structures");

    // Nested message content blocks
    let complex_message = Message::new(
        vec![
            MessageContentBlock::text("Simple text block"),
            MessageContentBlock::text("Another text block with more content"),
            MessageContentBlock::text(
                "Yet another block with even more detailed content for testing",
            ),
        ],
        Role::Assistant,
        "complex-task-id".to_string(),
    );

    group.bench_function("complex_message_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&complex_message).unwrap()))
    });

    let complex_message_json = serde_json::to_string(&complex_message).unwrap();
    group.bench_function("complex_message_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<Message>(&complex_message_json).unwrap()))
    });

    // Complex computer action with multiple coordinates
    let complex_action = ComputerAction::DragMouse {
        path: (0..100)
            .map(|i| Coordinates {
                x: i * 10,
                y: i * 10,
            })
            .collect(),
        button: Button::Left,
        hold_keys: Some(vec![
            "shift".to_string(),
            "ctrl".to_string(),
            "alt".to_string(),
        ]),
    };

    group.bench_function("complex_action_serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(&complex_action).unwrap()))
    });

    let complex_action_json = serde_json::to_string(&complex_action).unwrap();
    group.bench_function("complex_action_deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<ComputerAction>(&complex_action_json).unwrap()))
    });

    group.finish();
}

// Load testing for shared types
fn benchmark_load_testing(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_testing");
    group.measurement_time(Duration::from_secs(10));

    // Mixed operations load test
    group.bench_function("mixed_operations_load", |b| {
        b.iter(|| {
            let operations = 1000;
            let mut results = Vec::new();

            for i in 0..operations {
                match i % 5 {
                    0 => {
                        // Create and serialize task
                        let task = create_test_task();
                        let serialized = serde_json::to_string(&task).unwrap();
                        results.push(serialized);
                    }
                    1 => {
                        // Create message
                        let _message = create_test_message();
                        results.push("message_created".to_string());
                    }
                    2 => {
                        // Enum conversions
                        let status = TaskStatus::Running;
                        let status_str = status.to_string();
                        let _parsed: TaskStatus = status_str.parse().unwrap();
                        results.push(status_str);
                    }
                    3 => {
                        // Computer action serialization
                        let action = create_test_computer_action();
                        let serialized = serde_json::to_string(&action).unwrap();
                        results.push(serialized);
                    }
                    _ => {
                        // Simple operations
                        let now = Utc::now();
                        let formatted = now.to_rfc3339();
                        results.push(formatted);
                    }
                }
            }

            black_box(results)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_type_serialization,
    benchmark_type_validation,
    benchmark_enum_conversions,
    benchmark_collection_operations,
    benchmark_memory_usage,
    benchmark_complex_data_structures,
    benchmark_load_testing
);
criterion_main!(benches);
