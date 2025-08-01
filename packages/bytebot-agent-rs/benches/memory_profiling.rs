use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering},
};

use bytebot_shared_rs::types::task::{Role, Task, TaskPriority, TaskStatus, TaskType};
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use uuid::Uuid;

// Custom allocator to track memory usage
struct TrackingAllocator {
    allocated: AtomicUsize,
    deallocated: AtomicUsize,
    peak_usage: AtomicUsize,
}

impl TrackingAllocator {
    const fn new() -> Self {
        Self {
            allocated: AtomicUsize::new(0),
            deallocated: AtomicUsize::new(0),
            peak_usage: AtomicUsize::new(0),
        }
    }

    fn current_usage(&self) -> usize {
        self.allocated.load(Ordering::Relaxed) - self.deallocated.load(Ordering::Relaxed)
    }

    fn peak_usage(&self) -> usize {
        self.peak_usage.load(Ordering::Relaxed)
    }

    fn total_allocated(&self) -> usize {
        self.allocated.load(Ordering::Relaxed)
    }

    fn reset(&self) {
        self.allocated.store(0, Ordering::Relaxed);
        self.deallocated.store(0, Ordering::Relaxed);
        self.peak_usage.store(0, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            let size = layout.size();
            self.allocated.fetch_add(size, Ordering::Relaxed);

            // Update peak usage
            let current = self.current_usage();
            let mut peak = self.peak_usage.load(Ordering::Relaxed);
            while current > peak {
                match self.peak_usage.compare_exchange_weak(
                    peak,
                    current,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(x) => peak = x,
                }
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        self.deallocated.fetch_add(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator::new();

// Memory usage measurement utilities
struct MemorySnapshot {
    current_usage: usize,
    peak_usage: usize,
    total_allocated: usize,
}

impl MemorySnapshot {
    fn take() -> Self {
        Self {
            current_usage: ALLOCATOR.current_usage(),
            peak_usage: ALLOCATOR.peak_usage(),
            total_allocated: ALLOCATOR.total_allocated(),
        }
    }

    fn diff(&self, other: &MemorySnapshot) -> MemoryDiff {
        MemoryDiff {
            current_usage_diff: self.current_usage.saturating_sub(other.current_usage),
            peak_usage_diff: self.peak_usage.saturating_sub(other.peak_usage),
            total_allocated_diff: self.total_allocated.saturating_sub(other.total_allocated),
        }
    }
}

struct MemoryDiff {
    current_usage_diff: usize,
    peak_usage_diff: usize,
    total_allocated_diff: usize,
}

// Helper functions for creating test data
fn create_memory_test_task(i: usize) -> Task {
    Task {
        id: Uuid::new_v4().to_string(),
        description: format!("Memory test task {} with detailed description that takes up more memory to test allocation patterns", i),
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
            "output": format!("Task {} completed with detailed results", i),
            "metrics": {
                "duration": 1500 + i,
                "memory_used": 1024 * (i + 1),
                "cpu_usage": 0.75,
                "operations": vec![
                    format!("operation_{}_1", i),
                    format!("operation_{}_2", i),
                    format!("operation_{}_3", i),
                ]
            },
            "logs": vec![
                format!("Log entry 1 for task {}", i),
                format!("Log entry 2 for task {}", i),
                format!("Log entry 3 for task {}", i),
            ]
        })),
        model: json!({
            "provider": "anthropic",
            "name": "claude-3-sonnet-20240229",
            "title": "Claude 3 Sonnet",
            "parameters": {
                "temperature": 0.7,
                "max_tokens": 4000,
                "top_p": 0.9
            }
        }),
        user_id: Some(format!("user-{}", i % 100)),
    }
}

// Memory usage benchmarks for task creation
fn benchmark_task_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_memory_usage");

    for count in [1, 10, 100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("task_creation_memory", count),
            count,
            |b, &count| {
                b.iter_custom(|iters| {
                    let mut total_duration = std::time::Duration::new(0, 0);

                    for _ in 0..iters {
                        ALLOCATOR.reset();
                        let start_snapshot = MemorySnapshot::take();
                        let start_time = std::time::Instant::now();

                        let tasks: Vec<Task> =
                            (0..count).map(|i| create_memory_test_task(i)).collect();

                        let duration = start_time.elapsed();
                        let end_snapshot = MemorySnapshot::take();
                        let memory_diff = end_snapshot.diff(&start_snapshot);

                        // Store memory usage info (in a real scenario, you'd log this)
                        black_box((
                            tasks.len(),
                            memory_diff.total_allocated_diff,
                            memory_diff.peak_usage_diff,
                            memory_diff.current_usage_diff,
                        ));

                        total_duration += duration;
                    }

                    total_duration
                })
            },
        );
    }

    group.finish();
}

// Memory usage benchmarks for task serialization
fn benchmark_serialization_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization_memory_usage");

    for count in [1, 10, 100, 1000].iter() {
        let tasks: Vec<Task> = (0..*count).map(|i| create_memory_test_task(i)).collect();

        group.bench_with_input(
            BenchmarkId::new("serialize_tasks_memory", count),
            count,
            |b, _| {
                b.iter_custom(|iters| {
                    let mut total_duration = std::time::Duration::new(0, 0);

                    for _ in 0..iters {
                        ALLOCATOR.reset();
                        let start_snapshot = MemorySnapshot::take();
                        let start_time = std::time::Instant::now();

                        let serialized: Vec<String> = tasks
                            .iter()
                            .map(|task| serde_json::to_string(task).unwrap())
                            .collect();

                        let duration = start_time.elapsed();
                        let end_snapshot = MemorySnapshot::take();
                        let memory_diff = end_snapshot.diff(&start_snapshot);

                        black_box((
                            serialized.len(),
                            memory_diff.total_allocated_diff,
                            memory_diff.peak_usage_diff,
                        ));

                        total_duration += duration;
                    }

                    total_duration
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("deserialize_tasks_memory", count),
            count,
            |b, _| {
                let serialized_tasks: Vec<String> = tasks
                    .iter()
                    .map(|task| serde_json::to_string(task).unwrap())
                    .collect();

                b.iter_custom(|iters| {
                    let mut total_duration = std::time::Duration::new(0, 0);

                    for _ in 0..iters {
                        ALLOCATOR.reset();
                        let start_snapshot = MemorySnapshot::take();
                        let start_time = std::time::Instant::now();

                        let deserialized: Vec<Task> = serialized_tasks
                            .iter()
                            .map(|s| serde_json::from_str(s).unwrap())
                            .collect();

                        let duration = start_time.elapsed();
                        let end_snapshot = MemorySnapshot::take();
                        let memory_diff = end_snapshot.diff(&start_snapshot);

                        black_box((
                            deserialized.len(),
                            memory_diff.total_allocated_diff,
                            memory_diff.peak_usage_diff,
                        ));

                        total_duration += duration;
                    }

                    total_duration
                })
            },
        );
    }

    group.finish();
}

// Memory efficiency comparison benchmarks
fn benchmark_memory_efficiency_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency_comparison");

    // Compare different approaches to task creation
    group.bench_function("task_creation_with_defaults", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = std::time::Duration::new(0, 0);

            for _ in 0..iters {
                ALLOCATOR.reset();
                let start_time = std::time::Instant::now();

                let task = Task {
                    id: Uuid::new_v4().to_string(),
                    description: "Simple task".to_string(),
                    task_type: TaskType::default(),
                    status: TaskStatus::default(),
                    priority: TaskPriority::default(),
                    control: Role::default(),
                    created_at: Utc::now(),
                    created_by: Role::User,
                    scheduled_for: None,
                    updated_at: Utc::now(),
                    executed_at: None,
                    completed_at: None,
                    queued_at: None,
                    error: None,
                    result: None,
                    model: json!({"provider": "anthropic", "name": "claude-3-sonnet"}),
                    user_id: None,
                };

                let duration = start_time.elapsed();
                black_box(task);
                total_duration += duration;
            }

            total_duration
        })
    });

    group.bench_function("task_creation_with_builder", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = std::time::Duration::new(0, 0);

            for _ in 0..iters {
                ALLOCATOR.reset();
                let start_time = std::time::Instant::now();

                let model = json!({"provider": "anthropic", "name": "claude-3-sonnet"});
                let task = Task::new("Simple task".to_string(), model);

                let duration = start_time.elapsed();
                black_box(task);
                total_duration += duration;
            }

            total_duration
        })
    });

    group.finish();
}

// Memory leak detection benchmarks
fn benchmark_memory_leak_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_leak_detection");

    group.bench_function("repeated_task_operations", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = std::time::Duration::new(0, 0);

            for iter in 0..iters {
                let start_snapshot = MemorySnapshot::take();
                let start_time = std::time::Instant::now();

                // Perform operations that should not leak memory
                for i in 0..100 {
                    let task = create_memory_test_task(i);
                    let serialized = serde_json::to_string(&task).unwrap();
                    let _deserialized: Task = serde_json::from_str(&serialized).unwrap();
                }

                let duration = start_time.elapsed();
                let end_snapshot = MemorySnapshot::take();

                // Check for memory growth over iterations
                if iter > 0 {
                    let memory_diff = end_snapshot.diff(&start_snapshot);
                    // In a real test, you'd assert that memory usage doesn't grow unboundedly
                    black_box(memory_diff.current_usage_diff);
                }

                total_duration += duration;
            }

            total_duration
        })
    });

    group.finish();
}

// Resource efficiency benchmarks
fn benchmark_resource_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_efficiency");

    // Test memory usage vs. performance trade-offs
    group.bench_function("memory_vs_performance_tradeoff", |b| {
        b.iter_custom(|iters| {
            let mut total_duration = std::time::Duration::new(0, 0);

            for _ in 0..iters {
                ALLOCATOR.reset();
                let start_snapshot = MemorySnapshot::take();
                let start_time = std::time::Instant::now();

                // Create tasks with different memory footprints
                let small_tasks: Vec<Task> = (0..100)
                    .map(|i| Task {
                        id: format!("task-{}", i),
                        description: "Small task".to_string(),
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
                        model: json!({"provider": "anthropic"}),
                        user_id: None,
                    })
                    .collect();

                let large_tasks: Vec<Task> = (0..10).map(|i| create_memory_test_task(i)).collect();

                let duration = start_time.elapsed();
                let end_snapshot = MemorySnapshot::take();
                let memory_diff = end_snapshot.diff(&start_snapshot);

                black_box((
                    small_tasks.len(),
                    large_tasks.len(),
                    memory_diff.total_allocated_diff,
                    memory_diff.peak_usage_diff,
                ));

                total_duration += duration;
            }

            total_duration
        })
    });

    group.finish();
}

criterion_group!(
    memory_benches,
    benchmark_task_memory_usage,
    benchmark_serialization_memory_usage,
    benchmark_memory_efficiency_comparison,
    benchmark_memory_leak_detection,
    benchmark_resource_efficiency
);
criterion_main!(memory_benches);
