use bevy::prelude::*;
use virtual_joystick::*;

// Cooldown-based approach for tile-based joystick movement
//
// This example uses an explicit cooldown mechanism with separate handling for
// direction changes vs. cooldown expiration. The logic is more verbose but provides
// clearer control flow and easier debugging.
//
// **Approach:**
// - Press events: Fire immediately (like `just_pressed()`)
// - Drag events: 
//   - If direction changed: Process immediately and reset cooldown
//   - Else if cooldown expired: Process and reset cooldown
//   - Else: Decrement cooldown counter
// - Cooldown: Explicitly decremented each frame when not processing
//
// **Use case:** Tile-based games where you want more explicit control over the
// throttling logic, or when you need to add additional conditions to the movement
// processing (e.g., checking for obstacles, animations, etc.).

#[derive(Component)]
struct Player;

#[derive(Component)]
struct MovementController {
    intent: Vec2,
}

// Resource to track joystick input cooldown
#[derive(Resource)]
struct JoystickCooldown {
    frames_remaining: u32,
    last_direction: Vec2,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VirtualJoystickPlugin::<String>::default())
        .insert_resource(JoystickCooldown {
            // Throttle to once per N frames (adjust as needed)
            // For tile-based games, 10-15 frames is usually good
            frames_remaining: 0,
            last_direction: Vec2::ZERO,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (record_player_directional_input, move_player))
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn((Camera2d, Transform::from_xyz(0., 0., 5.0)));
    
    // Player sprite
    commands.spawn((
        Sprite {
            image: asset_server.load("Knob.png"),
            color: Color::srgb(0.5, 0.0, 0.5), // Purple
            custom_size: Some(Vec2::new(50., 50.)),
            ..default()
        },
        Player,
        MovementController { intent: Vec2::ZERO },
        Transform::default(),
    ));
    
    // Create joystick
    create_joystick(
        &mut commands,
        "UniqueJoystick".to_string(),
        asset_server.load("Knob.png"),
        asset_server.load("Outline.png"),
        None,
        None,
        Some(Color::srgba(1.0, 0.27, 0.0, 0.3)),
        Vec2::new(75., 75.),
        Vec2::new(150., 150.),
        Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            position_type: PositionType::Absolute,
            left: Val::Percent(0.),
            bottom: Val::Percent(0.),
            ..default()
        },
        (JoystickFloating, JoystickDigital8),
        NoAction,
    );
}

fn record_player_directional_input(
    input: Res<ButtonInput<KeyCode>>,
    mut joystick: MessageReader<VirtualJoystickEvent<String>>,
    mut controller_query: Query<&mut MovementController, With<Player>>,
    mut cooldown: ResMut<JoystickCooldown>,
) {
    // Collect directional input.
    let mut intent = Vec2::ZERO;

    // Keyboard input (WASD uses just_pressed, arrows use pressed)
    if input.just_pressed(KeyCode::KeyW) || input.pressed(KeyCode::ArrowUp) {
        intent.y += 1.0;
    }
    if input.just_pressed(KeyCode::KeyS) || input.pressed(KeyCode::ArrowDown) {
        intent.y -= 1.0;
    }
    if input.just_pressed(KeyCode::KeyA) || input.pressed(KeyCode::ArrowLeft) {
        intent.x -= 1.0;
    }
    if input.just_pressed(KeyCode::KeyD) || input.pressed(KeyCode::ArrowRight) {
        intent.x += 1.0;
    }

    // Process joystick events with throttling
    for j in joystick.read() {
        match j.get_type() {
            VirtualJoystickEventType::Press => {
                // Press events fire immediately (like just_pressed)
                let Vec2 { x, y } = j.axis();
                let mut joystick_intent = Vec2::ZERO;
                
                if *x > 0.2 {
                    joystick_intent.x += 1.0;
                } else if *x < -0.2 {
                    joystick_intent.x -= 1.0;
                }
                if *y > 0.2 {
                    joystick_intent.y += 1.0;
                } else if *y < -0.2 {
                    joystick_intent.y -= 1.0;
                }
                
                intent += joystick_intent;
                cooldown.last_direction = joystick_intent;
                cooldown.frames_remaining = 0; // Reset cooldown on press
            }
            VirtualJoystickEventType::Drag => {
                // Drag events: throttle based on frames and direction change
                let Vec2 { x, y } = j.axis();
                let mut joystick_intent = Vec2::ZERO;
                
                if *x > 0.2 {
                    joystick_intent.x += 1.0;
                } else if *x < -0.2 {
                    joystick_intent.x -= 1.0;
                }
                if *y > 0.2 {
                    joystick_intent.y += 1.0;
                } else if *y < -0.2 {
                    joystick_intent.y -= 1.0;
                }
                
                // Check if direction changed (reset cooldown) or cooldown expired
                let direction_changed = joystick_intent != cooldown.last_direction;
                
                if direction_changed {
                    // Direction changed: process immediately and reset cooldown
                    intent += joystick_intent;
                    cooldown.last_direction = joystick_intent;
                    cooldown.frames_remaining = 12; // Set cooldown (adjust as needed)
                } else if cooldown.frames_remaining == 0 {
                    // Same direction and cooldown expired: process and reset cooldown
                    intent += joystick_intent;
                    cooldown.frames_remaining = 12; // Set cooldown (adjust as needed)
                } else {
                    // Still in cooldown: decrement counter
                    cooldown.frames_remaining -= 1;
                }
            }
            VirtualJoystickEventType::Up => {
                // Up events: reset cooldown
                cooldown.last_direction = Vec2::ZERO;
                cooldown.frames_remaining = 0;
            }
        }
    }

    // Apply movement intent to controllers.
    for mut controller in &mut controller_query {
        controller.intent = intent;
    }
}

// System to actually move the player based on intent
fn move_player(
    mut player_query: Query<(&mut Transform, &MovementController), With<Player>>,
    time: Res<Time>,
) {
    for (mut transform, controller) in &mut player_query {
        let speed = 100.0;
        transform.translation.x += controller.intent.x * speed * time.delta_secs();
        transform.translation.y += controller.intent.y * speed * time.delta_secs();
    }
}

