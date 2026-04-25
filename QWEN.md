# Virtual Joystick Plugin for Bevy 0.16

## Project Overview

The virtual_joystick crate is a Bevy plugin that provides on-screen, touch-based joystick controls for games. It supports both touch input (mobile/web) and mouse input (desktop) with different joystick behaviors (Fixed, Floating, Dynamic) and customizable actions.

## Event Management Architecture

### Core Event Types

#### InputEvent (Internal)
- `StartDrag { id: u64, pos: Vec2, is_mouse: bool }` - When a touch/mouse drag begins
- `Dragging { id: u64, pos: Vec2, is_mouse: bool }` - During continuous drag motion
- `EndDrag { id: u64, pos: Vec2, is_mouse: bool }` - When a drag ends

#### VirtualJoystickEvent (Public)
- `VirtualJoystickEvent<S: VirtualJoystickID>` - The main event that applications consume
  - Contains: `id`, `event type` (Press, Drag, Up), `value` (raw position), `delta` (normalized -1 to 1)

### Event Detection Pipeline

1. **Input Detection (`update_input` system)**:
   - Runs in `PreUpdate` stage
   - Monitors both `ButtonInput<MouseButton>` for mouse and `Res<Touches>` for touch
   - Detects when touch/mouse enters the joystick's interaction area (defined by `JoystickInteractionArea`)
   - Creates `TouchState` to track ongoing interaction
   - Handles press/drag/release states for both mouse and touch

2. **State Management**:
   - `VirtualJoystickState` component stores:
     - `touch_state: Option<TouchState>` - current touch/mouse state
     - `just_released: bool` - indicates release happened this frame
     - `base_offset: Vec2` - offset of the joystick from its original position
     - `delta: Vec2` - normalized joystick position (-1 to 1)

3. **Behavior Processing** (runs in custom schedules):
   - `UpdateKnobDelta` schedule: Calculates delta based on touch position
   - `ConstrainKnobDelta` schedule: Applies constraints (dead zones, axis restrictions)
   - Different behaviors (Fixed, Floating, Dynamic) calculate the delta differently

4. **Event Firing (`update_fire_events` system)**:
   - Runs in `FireEvents` schedule (executed in `Update`)
   - Converts internal states to `VirtualJoystickEvent` instances
   - Each frame during interaction, fires appropriate events
   - Events are written to Bevy's event queue for consumption by other systems

5. **UI Updates (`update_ui` system)**:
   - Runs in `UpdateUI` schedule (executed in `Update`)
   - Updates the visual position of the joystick knob based on the computed `delta`

### Key Systems and Execution Flow

```
PreUpdate Stage:
├── update_missing_state (ensures all joysticks have VirtualJoystickState)
├── update_input (detects mouse/touch and updates touch states)

Custom Schedules (executed in Update):
├── UpdateKnobDelta: update_behavior_knob_delta (calculates raw delta)
├── ConstrainKnobDelta: update_behavior_constraints (applies behavior constraints)
├── FireEvents: update_fire_events (creates VirtualJoystickEvent instances)
├── UpdateUI: update_behavior and update_ui (updates visuals and applies behaviors)
```

### Touch/Mouse Event Detection Process

1. **Detection Phase** (in `update_input`):
   - Checks if touches are within joystick interaction area
   - If mouse, checks if left button is pressed and cursor is within area
   - Creates TouchState when interaction begins

2. **Tracking Phase**:
   - Updates current position from window cursor for mouse
   - Updates current position from touch for touch input
   - Maintains persistent touch state between frames

3. **Release Phase**:
   - Detects when buttons are released or touches end
   - Sets `just_released = true` and clears touch state

### Joystick Behaviors and Their Impact

- **JoystickFixed**: Joystick stays in fixed position, knob moves within the circle
- **JoystickFloating**: Joystick moves to touch location on press, knob moves relative to base
- **JoystickDynamic**: Joystick base moves with drag, knob moves relative to base

Each behavior implements different logic in the `update_at_delta_stage` method to calculate the `delta` value.

### Event Propagation to Applications

1. Applications insert the `VirtualJoystickPlugin::<YourIDType>::default()` into their app
2. Plugin registers the `VirtualJoystickEvent<YourIDType>` event type
3. Applications can read these events using `EventReader<VirtualJoystickEvent<YourIDType>>`
4. Each event contains the joystick ID and normalized delta values for input processing

## Bevy 0.17 Upgrade Considerations

The main changes for Bevy 0.17 will likely involve:

1. **Event System Changes**: Bevy 0.17 introduces significant changes to the event system and input handling
2. **Schedule Management**: The custom schedules and manual schedule execution may need updates
3. **Input API**: Touch and mouse input APIs may have changed
4. **UI System**: UI component and node updates may affect the visual feedback system

The core architecture of detecting input → updating state → calculating deltas → firing events → updating visuals should remain valid, but the specific API calls and event handling mechanisms will need to be updated to match Bevy 0.17's new patterns.

## Component Structure

- `VirtualJoystickNode<S>`: Main component with behavior/action configuration
- `VirtualJoystickState`: Runtime state tracking
- `VirtualJoystickUIKnob`: Marks the knob UI element
- `VirtualJoystickUIBackground`: Marks the background UI element  
- `JoystickInteractionArea`: Defines touch/mouse interaction area