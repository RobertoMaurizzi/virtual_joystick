use bevy::prelude::*;
use virtual_joystick::*;

// Throttle-based approach for tile-based joystick movement
//
// This example uses a simple throttling mechanism where joystick input is processed
// every N frames (default: 12 frames). The throttling logic combines direction change
// detection and cooldown expiration into a single condition check.
//
// **Approach:**
// - Press events: Fire immediately (like `just_pressed()`)
// - Drag events: Process when direction changes OR cooldown expires
// - Cooldown: Resets to N frames after each processed movement
//
// **Use case:** Simple tile-based games where you want consistent movement speed
// regardless of frame rate. The combined condition makes the code more compact.

#[derive(Component)]
struct Player;

#[derive(Component)]
struct MovementController {
    intent: Vec2,
}

#[derive(Resource)]
struct JoystickThrottle {
    frames_remaining: u32,
    last_direction: Vec2,
}

fn record_player_directional_input(
    input: Res<ButtonInput<KeyCode>>,
    mut joystick: MessageReader<VirtualJoystickEvent<String>>,
    mut controller_query: Query<&mut MovementController, With<Player>>,
    mut throttle: ResMut<JoystickThrottle>,
) {
    // Collect directional input.
    let mut intent = Vec2::ZERO;

    // Keyboard input (existing code)
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
        let Vec2 { x, y } = j.axis();
        let mut joystick_intent = Vec2::ZERO;
        
        // Convert joystick axis to direction (same logic as before)
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
        
        match j.get_type() {
            VirtualJoystickEventType::Press => {
                // Press events fire immediately (like just_pressed)
                intent += joystick_intent;
                throttle.last_direction = joystick_intent;
                throttle.frames_remaining = 0; // Reset cooldown
            }
            VirtualJoystickEventType::Drag => {
                // Throttle drag events: only process every N frames or when direction changes
                let direction_changed = joystick_intent != throttle.last_direction;
                
                if direction_changed || throttle.frames_remaining == 0 {
                    // Direction changed or cooldown expired: process movement
                    intent += joystick_intent;
                    throttle.last_direction = joystick_intent;
                    throttle.frames_remaining = 12; // Set cooldown (adjust 12 to your preference)
                } else {
                    // Still in cooldown: skip this frame
                    throttle.frames_remaining -= 1;
                }
            }
            VirtualJoystickEventType::Up => {
                // Reset on release
                throttle.last_direction = Vec2::ZERO;
                throttle.frames_remaining = 0;
            }
        }
    }

    // Apply movement intent to controllers.
    for mut controller in &mut controller_query {
        controller.intent = intent;
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(VirtualJoystickPlugin::<String>::default())
        .insert_resource(JoystickThrottle {
            frames_remaining: 0,
            last_direction: Vec2::ZERO,
        })
        .add_systems(Startup, |mut commands: Commands, asset_server: Res<AssetServer>| {
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
        })
        .add_systems(Update, (record_player_directional_input, move_player))
        .run();
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

