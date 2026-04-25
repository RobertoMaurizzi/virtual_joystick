# Bevy 0.16 → 0.17 Upgrade Summary

This document describes the changes made to upgrade the virtual joystick plugin from Bevy 0.16 to Bevy 0.17, focusing on understanding **what changed in Bevy** and **how we fixed it**.

## 1. Event System → Message System

### What Changed in Bevy 0.17

Bevy 0.17 introduced a new **Message** system that replaces the old **Event** system. The key differences:

- **Old (0.16)**: `Event<T>` with `EventReader<T>` and `EventWriter<T>`
- **New (0.17)**: `Message<T>` with `MessageReader<T>` and `MessageWriter<T>`
- Types can now derive **both** `Event` and `Message` for backward compatibility
- Registration changed from `add_event::<T>()` to `add_message::<T>()`

### What We Changed

**File: `src/lib.rs`**
```rust
// Before (0.16):
app.add_event::<VirtualJoystickEvent<S>>()
   .add_event::<InputEvent>()

// After (0.17):
app.add_message::<VirtualJoystickEvent<S>>()
   .add_message::<InputEvent>()
```

**File: `examples/simple.rs`**
```rust
// Before (0.16):
fn joystick_event_handler(mut events: EventReader<VirtualJoystickEvent<UniqueJoystick>>) {

// After (0.17):
fn joystick_event_handler(mut events: MessageReader<VirtualJoystickEvent<UniqueJoystick>>) {
```

**File: `src/systems.rs` (update_fire_events)**
```rust
// Before (0.16):
mut events: EventWriter<VirtualJoystickEvent<S>>,
mut input_events: EventWriter<InputEvent>,

// After (0.17):
mut events: MessageWriter<VirtualJoystickEvent<S>>,
mut input_events: MessageWriter<InputEvent>,
```

### Why This Was Necessary

The Message system provides better performance and clearer semantics. Events are now a special case of Messages with additional features like persistence and filtering.

---

## 2. ComputedNode API Changes

### What Changed in Bevy 0.17

`ComputedNode` fields changed from **methods** to **direct field access**:

- **Old (0.16)**: `computed_node.size()` (method call)
- **New (0.17)**: `computed_node.size` (field access)
- **Old (0.16)**: `computed_node.inverse_scale_factor()` (method call)
- **New (0.17)**: `computed_node.inverse_scale_factor` (field access)

### What We Changed

**Files: `src/systems.rs`, `src/behavior.rs`**

All occurrences of:
```rust
// Before (0.16):
computed_node.size()
computed_node.inverse_scale_factor()

// After (0.17):
computed_node.size
computed_node.inverse_scale_factor
```

### Why This Was Necessary

Bevy simplified the API by making these computed values directly accessible as fields rather than requiring method calls. This is a performance optimization and API simplification.

---

## 3. UI Node Creation Changes

### What Changed in Bevy 0.17

The UI bundle system changed:
- **Old (0.16)**: `ImageBundle` for UI images
- **New (0.17)**: `ImageNode` for UI images

### What We Changed

**File: `src/utils.rs`**
```rust
// Before (0.16):
ImageBundle {
    image: UiImage::new(handle),
    style: Style { ... },
    ..default()
}

// After (0.17):
ImageNode {
    image: UiImage::new(handle),
    style: Style { ... },
    ..default()
}
```

### Why This Was Necessary

Bevy refactored UI bundles to be more consistent and type-safe. `ImageNode` is now the standard way to create image UI elements.

---

## 4. Coordinate System Issues (The Main Runtime Bugs)

### The Problem

After the initial upgrade, the joystick had several issues:
1. **Off-center positioning**: UI elements were drawn in wrong positions
2. **Input detection only in upper-left quadrant**: Clicks only worked in part of the window
3. **Joystick resetting on release**: Floating joystick would jump back to center

### Root Cause: Coordinate System Mismatch

Bevy's UI system uses **screen coordinates** (center at `(0, 0)`), while `window.cursor_position()` returns **window coordinates** (top-left at `(0, 0)`).

#### Screen Coordinates (UI System)
- Origin: Center of window at `(0, 0)`
- X-axis: Negative left, positive right
- Y-axis: Negative bottom, positive top
- Example: For a 1280×720 window, coordinates range from `(-640, -360)` to `(640, 360)`

#### Window Coordinates (Cursor Position)
- Origin: Top-left corner at `(0, 0)`
- X-axis: Increases rightward
- Y-axis: Increases downward
- Example: For a 1280×720 window, coordinates range from `(0, 0)` to `(1280, 720)`

### What We Fixed

#### Fix 1: Input Detection Coordinate Conversion

**File: `src/systems.rs` (update_input system)**

```rust
// Before:
if let Some(mouse_pos) = window.cursor_position() {
    if parent_rect.contains(mouse_pos) {  // ❌ Wrong coordinate system!
        // ...
    }
}

// After:
if let Some(mouse_pos_window) = window.cursor_position() {
    // Convert window coordinates to screen coordinates
    let window_size = Vec2::new(window.width(), window.height());
    let mouse_pos_screen = mouse_pos_window - window_size * 0.5;
    if parent_rect.contains(mouse_pos_screen) {  // ✅ Correct!
        // ...
    }
}
```

**Why**: `parent_rect` is calculated using `GlobalTransform` which is in screen coordinates, but `cursor_position()` returns window coordinates. We convert by subtracting half the window size.

#### Fix 2: UI Positioning Calculation

**File: `src/systems.rs` (update_ui system)**

The positioning calculation needed to account for the coordinate system conversion:

```rust
// base_offset is in screen coordinates (offset from parent center)
// Parent center is at (0, 0) in screen coords
// Parent top-left in screen coords = parent_center_screen - parent_size * 0.5
// To convert screen coord to parent-relative: screen_coord - parent_top_left
// Base center in screen coords = parent_center_screen + base_offset
// Base center in parent-relative = (parent_center_screen + base_offset) - (parent_center_screen - parent_size * 0.5)
//                                 = base_offset + parent_size * 0.5

let base_center_in_parent = joystick_state.base_offset + parent_size * 0.5;
let base_left = base_center_in_parent.x - base_size.x * 0.5;
let base_top = base_center_in_parent.y - base_size.y * 0.5;
```

**Why**: UI elements use `PositionType::Absolute` which positions relative to the parent's top-left corner. Since `base_offset` is in screen coordinates (relative to parent center), we need to add `parent_size * 0.5` to convert to parent-relative coordinates.

#### Fix 3: Floating Joystick Base Position Calculation

**File: `src/behavior.rs` (JoystickFloating::update_at_delta_stage)**

The floating joystick behavior needed to calculate the base position correctly:

```rust
// Get parent (joystick entity) transform and size
let parent_size = joystick_node.size() * joystick_node.inverse_scale_factor;
let parent_center_screen = joystick_global_transform.translation().xy() * joystick_node.inverse_scale_factor;

// Calculate base_offset: offset from parent center to touch start
base_offset = touch_state.start - parent_center_screen;

// Calculate base center from parent center + base_offset
let base_center_screen = parent_center_screen + base_offset;
```

**Why**: Previously, we tried to get the base position from child entities' `GlobalTransform`, but these weren't available when the system ran. Instead, we calculate it from the parent's transform and the stored `base_offset`.

#### Fix 4: Keep Floating Joystick Position on Release

**File: `src/behavior.rs` (JoystickFloating::update_at_delta_stage)**

```rust
// Before:
} else if joystick_state.just_released {
    base_offset = Vec2::ZERO;  // ❌ Resets to center
    assign_base_offset = true;
}

// After:
} else if joystick_state.just_released {
    // For floating joystick, keep base_offset where it is (don't reset to zero)
    base_offset = joystick_state.base_offset;  // ✅ Keep current position
    // Don't assign - keep it where it was
}
```

**Why**: Floating joysticks should stay where they were placed until the next click, not reset to center on release.

---

## 5. System Query Filtering

### What Changed

We needed to ensure queries properly filter for the correct joystick entities.

### What We Changed

**File: `src/systems.rs`**

Added `With<VirtualJoystickNode<S>>` filter to queries:

```rust
// update_input system:
pub fn update_input<S: VirtualJoystickID>(
    mut joysticks: Query<(
        Entity,
        &ComputedNode,
        &GlobalTransform,
        &mut VirtualJoystickState,
    ), With<VirtualJoystickNode<S>>>,  // ✅ Added filter
    // ...
)

// update_ui system:
pub fn update_ui(
    joysticks: Query<(&VirtualJoystickState, &Children, &ComputedNode, &GlobalTransform), With<VirtualJoystickNode<S>>>,  // ✅ Added filter
    // ...
)
```

**Why**: This ensures we only process entities that are actually joystick nodes, preventing incorrect entity processing.

---

## 6. Input Detection Improvements

### What We Changed

**File: `src/systems.rs` (update_input)**

Changed from `just_pressed` to `pressed` for continuous mouse tracking:

```rust
// Before:
if mouse_buttons.just_pressed(MouseButton::Left) {  // Only fires once

// After:
if mouse_buttons.pressed(MouseButton::Left) {  // Fires continuously while held
```

**Why**: For floating joysticks, we need to detect clicks anywhere in the parent area, not just on the first frame. Using `pressed` allows detection even if the mouse was already down when entering the area.

---

## 7. Example Code Updates

### What We Changed

**File: `examples/simple.rs`**

```rust
// Before (0.16):
app.add_plugins(EguiPlugin::default())

// After (0.17):
app.add_plugins(EguiPlugin {
    enable_multipass_for_primary_context: true
})
```

**Why**: Bevy 0.17 changed the default configuration for `EguiPlugin` and requires explicit configuration.

---

## Summary of Key Takeaways

1. **Events → Messages**: Bevy 0.17 uses a new Message system. Update `add_event` to `add_message` and `EventReader/Writer` to `MessageReader/Writer`.

2. **ComputedNode API**: Field access instead of method calls (`size` instead of `size()`).

3. **Coordinate Systems**: Bevy UI uses screen coordinates (center origin), while `cursor_position()` uses window coordinates (top-left origin). Always convert between them when comparing positions.

4. **UI Positioning**: When using `PositionType::Absolute`, positions are relative to the parent's top-left. Convert from screen coordinates by adding `parent_size * 0.5`.

5. **System Timing**: Some components (like child `GlobalTransform`) may not be available when systems run. Calculate positions from parent transforms instead.

6. **Input Detection**: Use `pressed()` for continuous detection, `just_pressed()` for one-time events.

These changes ensure the plugin works correctly with Bevy 0.17's new APIs and coordinate systems.

