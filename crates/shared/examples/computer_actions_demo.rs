use shared::types::computer_action::*;

fn main() {
    println!("ByteBot Computer Action Types Demo");
    println!("==================================");

    // Create various computer actions
    let actions = vec![
        // Mouse actions
        ComputerAction::MoveMouse {
            coordinates: Coordinates { x: 100, y: 200 },
        },
        ComputerAction::ClickMouse {
            coordinates: Some(Coordinates { x: 150, y: 250 }),
            button: Button::Left,
            hold_keys: Some(vec!["ctrl".to_string()]),
            click_count: 2,
        },
        ComputerAction::DragMouse {
            path: vec![
                Coordinates { x: 0, y: 0 },
                Coordinates { x: 100, y: 100 },
                Coordinates { x: 200, y: 200 },
            ],
            button: Button::Left,
            hold_keys: None,
        },
        ComputerAction::Scroll {
            coordinates: Some(Coordinates { x: 300, y: 400 }),
            direction: ScrollDirection::Down,
            scroll_count: 3,
            hold_keys: None,
        },
        // Keyboard actions
        ComputerAction::TypeText {
            text: "Hello, ByteBot!".to_string(),
            delay: Some(50),
            sensitive: Some(false),
        },
        ComputerAction::PressKeys {
            keys: vec!["ctrl".to_string(), "c".to_string()],
            press: Press::Down,
        },
        ComputerAction::PasteText {
            text: "Pasted content".to_string(),
        },
        // System actions
        ComputerAction::Screenshot,
        ComputerAction::CursorPosition,
        ComputerAction::Application {
            application: Application::Firefox,
        },
        ComputerAction::Wait { duration: 1000 },
        // File operations
        ComputerAction::WriteFile {
            path: "/tmp/test.txt".to_string(),
            data: "SGVsbG8gV29ybGQ=".to_string(), // "Hello World" in base64
        },
        ComputerAction::ReadFile {
            path: "/tmp/test.txt".to_string(),
        },
    ];

    // Demonstrate serialization and validation
    for (i, action) in actions.iter().enumerate() {
        println!("\nAction {}: {:?}", i + 1, action);

        // Validate the action
        match action.validate() {
            Ok(()) => println!("✓ Validation: PASSED"),
            Err(e) => println!("✗ Validation: FAILED - {e}"),
        }

        // Serialize to JSON
        match serde_json::to_string_pretty(action) {
            Ok(json) => println!("JSON:\n{json}"),
            Err(e) => println!("✗ Serialization: FAILED - {e}"),
        }
    }

    // Demonstrate helper functions
    println!("\n\nHelper Functions Demo:");
    println!("=====================");

    // Using helper functions with validation
    match ComputerAction::move_mouse(Coordinates { x: 50, y: 75 }) {
        Ok(action) => println!("✓ Created move_mouse action: {action:?}"),
        Err(e) => println!("✗ Failed to create move_mouse action: {e}"),
    }

    match ComputerAction::click_mouse(Some(Coordinates { x: 100, y: 150 }), Button::Right, 1, None)
    {
        Ok(action) => println!("✓ Created click_mouse action: {action:?}"),
        Err(e) => println!("✗ Failed to create click_mouse action: {e}"),
    }

    match ComputerAction::type_text("Test text".to_string(), Some(100), None) {
        Ok(action) => println!("✓ Created type_text action: {action:?}"),
        Err(e) => println!("✗ Failed to create type_text action: {e}"),
    }

    // Demonstrate validation failures
    println!("\nValidation Failures Demo:");
    println!("=========================");

    match ComputerAction::move_mouse(Coordinates { x: -1, y: 100 }) {
        Ok(_) => println!("✗ This should have failed!"),
        Err(e) => println!("✓ Expected validation error: {e}"),
    }

    match ComputerAction::click_mouse(None, Button::Left, 0, None) {
        Ok(_) => println!("✗ This should have failed!"),
        Err(e) => println!("✓ Expected validation error: {e}"),
    }

    match ComputerAction::type_text("".to_string(), None, None) {
        Ok(_) => println!("✗ This should have failed!"),
        Err(e) => println!("✓ Expected validation error: {e}"),
    }
}
