use std::time::Duration;

use bytebot_shared_rs::types::computer_action::{
    Application, Button, ComputerAction, Coordinates, Press, ScrollDirection,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::Value;
use tokio::runtime::Runtime;

// Mock automation service for isolated benchmarks
#[derive(Debug, Clone)]
struct MockAutomationService;

impl MockAutomationService {
    fn new() -> Self {
        Self
    }

    async fn execute_action_mock(&self, action: ComputerAction) -> Result<Value, String> {
        // Simulate processing time based on action complexity
        let delay_ms = match &action {
            ComputerAction::Screenshot => 50, // Screenshot is expensive
            ComputerAction::ReadFile { .. } => 10,
            ComputerAction::WriteFile { .. } => 15,
            ComputerAction::TypeText { text, .. } => text.len() as u64 / 10, // Simulate typing speed
            ComputerAction::MoveMouse { .. } => 1,
            ComputerAction::ClickMouse { .. } => 2,
            ComputerAction::Scroll { .. } => 5,
            ComputerAction::Application { .. } => 100, // App switching is slow
            _ => 1,
        };

        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        match action {
            ComputerAction::Screenshot => Ok(serde_json::json!({
                "success": true,
                "image": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="
            })),
            ComputerAction::ReadFile { path } => Ok(serde_json::json!({
                "success": true,
                "data": format!("Mock file content for {}", path),
                "mediaType": "text/plain",
                "name": "test.txt",
                "size": 100
            })),
            ComputerAction::CursorPosition => Ok(serde_json::json!({
                "success": true,
                "x": 500,
                "y": 300
            })),
            _ => Ok(serde_json::json!({"success": true})),
        }
    }
}

// Helper functions for creating test actions
fn create_mouse_actions() -> Vec<ComputerAction> {
    vec![
        ComputerAction::MoveMouse {
            coordinates: Coordinates { x: 100, y: 100 },
        },
        ComputerAction::ClickMouse {
            coordinates: Some(Coordinates { x: 200, y: 200 }),
            button: Button::Left,
            hold_keys: None,
            click_count: 1,
        },
        ComputerAction::DragMouse {
            path: vec![
                Coordinates { x: 100, y: 100 },
                Coordinates { x: 200, y: 200 },
                Coordinates { x: 300, y: 300 },
            ],
            button: Button::Left,
            hold_keys: None,
        },
        ComputerAction::Scroll {
            coordinates: Some(Coordinates { x: 400, y: 400 }),
            direction: ScrollDirection::Down,
            scroll_count: 3,
            hold_keys: None,
        },
    ]
}

fn create_keyboard_actions() -> Vec<ComputerAction> {
    vec![
        ComputerAction::TypeText {
            text: "Hello, World!".to_string(),
            delay: None,
            sensitive: None,
        },
        ComputerAction::TypeKeys {
            keys: vec!["ctrl".to_string(), "c".to_string()],
            delay: None,
        },
        ComputerAction::PressKeys {
            keys: vec!["enter".to_string()],
            press: Press::Down,
        },
        ComputerAction::PasteText {
            text: "Pasted content".to_string(),
        },
    ]
}

fn create_file_actions() -> Vec<ComputerAction> {
    vec![
        ComputerAction::ReadFile {
            path: "/tmp/test.txt".to_string(),
        },
        ComputerAction::WriteFile {
            path: "/tmp/output.txt".to_string(),
            data: "Test file content".to_string(),
        },
    ]
}

fn create_screen_actions() -> Vec<ComputerAction> {
    vec![
        ComputerAction::Screenshot,
        ComputerAction::CursorPosition,
        ComputerAction::Application {
            application: Application::Firefox,
        },
    ]
}

// Individual action benchmarks
fn benchmark_mouse_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = MockAutomationService::new();
    let actions = create_mouse_actions();

    let mut group = c.benchmark_group("mouse_operations");

    for (i, action) in actions.into_iter().enumerate() {
        let action_name = match &action {
            ComputerAction::MoveMouse { .. } => "move_mouse",
            ComputerAction::ClickMouse { .. } => "click_mouse",
            ComputerAction::DragMouse { .. } => "drag_mouse",
            ComputerAction::Scroll { .. } => "scroll",
            _ => "unknown",
        };

        group.bench_function(action_name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(service.execute_action_mock(action.clone()).await.unwrap())
                })
            })
        });
    }

    group.finish();
}

fn benchmark_keyboard_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = MockAutomationService::new();
    let actions = create_keyboard_actions();

    let mut group = c.benchmark_group("keyboard_operations");

    for action in actions.into_iter() {
        let action_name = match &action {
            ComputerAction::TypeText { .. } => "type_text",
            ComputerAction::TypeKeys { .. } => "type_keys",
            ComputerAction::PressKeys { .. } => "press_keys",
            ComputerAction::PasteText { .. } => "paste_text",
            _ => "unknown",
        };

        group.bench_function(action_name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(service.execute_action_mock(action.clone()).await.unwrap())
                })
            })
        });
    }

    group.finish();
}

fn benchmark_file_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = MockAutomationService::new();
    let actions = create_file_actions();

    let mut group = c.benchmark_group("file_operations");

    for action in actions.into_iter() {
        let action_name = match &action {
            ComputerAction::ReadFile { .. } => "read_file",
            ComputerAction::WriteFile { .. } => "write_file",
            _ => "unknown",
        };

        group.bench_function(action_name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(service.execute_action_mock(action.clone()).await.unwrap())
                })
            })
        });
    }

    group.finish();
}

fn benchmark_screen_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = MockAutomationService::new();
    let actions = create_screen_actions();

    let mut group = c.benchmark_group("screen_operations");

    for action in actions.into_iter() {
        let action_name = match &action {
            ComputerAction::Screenshot => "screenshot",
            ComputerAction::CursorPosition => "cursor_position",
            ComputerAction::Application { .. } => "application_switch",
            _ => "unknown",
        };

        group.bench_function(action_name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(service.execute_action_mock(action.clone()).await.unwrap())
                })
            })
        });
    }

    group.finish();
}

// Action serialization benchmarks
fn benchmark_action_serialization(c: &mut Criterion) {
    let actions = vec![
        ComputerAction::MoveMouse {
            coordinates: Coordinates { x: 100, y: 100 },
        },
        ComputerAction::TypeText {
            text: "This is a test string for serialization benchmarking".to_string(),
            delay: Some(50),
            sensitive: None,
        },
        ComputerAction::DragMouse {
            path: vec![
                Coordinates { x: 0, y: 0 },
                Coordinates { x: 100, y: 100 },
                Coordinates { x: 200, y: 200 },
                Coordinates { x: 300, y: 300 },
            ],
            button: Button::Left,
            hold_keys: Some(vec!["shift".to_string()]),
        },
    ];

    let mut group = c.benchmark_group("action_serialization");

    for (i, action) in actions.iter().enumerate() {
        group.bench_function(&format!("serialize_action_{}", i), |b| {
            b.iter(|| black_box(serde_json::to_string(action).unwrap()))
        });

        let serialized = serde_json::to_string(action).unwrap();
        group.bench_function(&format!("deserialize_action_{}", i), |b| {
            b.iter(|| black_box(serde_json::from_str::<ComputerAction>(&serialized).unwrap()))
        });
    }

    group.finish();
}

// Text input performance benchmarks
fn benchmark_text_input_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let service = MockAutomationService::new();

    let mut group = c.benchmark_group("text_input_performance");

    for text_length in [10, 100, 1000, 10000].iter() {
        let text = "a".repeat(*text_length);
        let action = ComputerAction::TypeText {
            text: text.clone(),
            delay: None,
            sensitive: None,
        };

        group.bench_with_input(
            BenchmarkId::new("type_text_length", text_length),
            text_length,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        black_box(service.execute_action_mock(action.clone()).await.unwrap())
                    })
                })
            },
        );
    }

    group.finish();
}

// Memory usage benchmarks for automation
fn benchmark_automation_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("automation_memory_usage");

    for count in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("action_creation_memory", count),
            count,
            |b, &count| {
                b.iter(|| {
                    let actions: Vec<ComputerAction> = (0..count)
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

criterion_group!(
    benches,
    benchmark_mouse_operations,
    benchmark_keyboard_operations,
    benchmark_file_operations,
    benchmark_screen_operations,
    benchmark_action_serialization,
    benchmark_text_input_performance,
    benchmark_automation_memory_usage
);
criterion_main!(benches);
