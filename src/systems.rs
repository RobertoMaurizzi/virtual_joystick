use std::sync::Arc;

use bevy::{
    camera::visibility::Visibility,
    ecs::{
        entity::Entity,
        message::MessageWriter,
        query::{With, Without},
        system::{Query, Res},
        world::World,
    },
    input::{mouse::MouseButton, touch::Touches, ButtonInput},
    math::{Rect, Vec2, Vec3Swizzles},
    prelude::Children,
    transform::components::GlobalTransform,
    ui::{ComputedNode, Node, PositionType, Val},
    window::{PrimaryWindow, Window},
};

use crate::{
    components::{
        JoystickInteractionArea, TouchState, VirtualJoystickState, VirtualJoystickUIBackground,
        VirtualJoystickUIKnob,
    },
    VirtualJoystickEvent, VirtualJoystickEventType, VirtualJoystickID, VirtualJoystickNode,
};

pub fn update_missing_state<S: VirtualJoystickID>(world: &mut World) {
    let mut joysticks = world.query::<(Entity, &VirtualJoystickNode<S>)>();
    let mut joystick_entities: Vec<Entity> = Vec::new();
    for (joystick_entity, _) in joysticks.iter(world) {
        joystick_entities.push(joystick_entity);
    }
    for joystick_entity in joystick_entities {
        let has_state = world.get::<VirtualJoystickState>(joystick_entity).is_some();
        if !has_state {
            world
                .entity_mut(joystick_entity)
                .insert(VirtualJoystickState::default());
        }
    }
}

pub fn update_input<S: VirtualJoystickID>(
    mut joysticks: Query<(
        Entity,
        &ComputedNode,
        &GlobalTransform,
        &mut VirtualJoystickState,
    ), With<VirtualJoystickNode<S>>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    children_query: Query<&Children>,
    interaction_areas: Query<(&ComputedNode, &GlobalTransform), With<JoystickInteractionArea>>,
    base_backgrounds_query: Query<(&ComputedNode, &GlobalTransform), With<VirtualJoystickUIBackground>>,
    base_background_nodes: Query<&Node, With<VirtualJoystickUIBackground>>,
    parent_nodes: Query<&Node, With<VirtualJoystickNode<S>>>,
) {
    for (joystick_entity, joystick_node, joystick_global_transform, mut joystick_state) in
        &mut joysticks
    {
        joystick_state.just_released = false;
        if let Some(touch_state) = &mut joystick_state.touch_state {
            touch_state.just_pressed = false;
        }

        if joystick_state.touch_state.is_none() {
            // Try to get base rect from children (for fixed joysticks)
            // Note: ComputedNode/GlobalTransform may not be available in PreUpdate, so we calculate from Node style
            let mut interaction_rect: Option<Rect> = None;
            if let Ok(children) = children_query.get(joystick_entity) {
                for &child in children.iter() {
                    // First try interaction area (if it has ComputedNode available)
                    if let Ok((base_node, base_transform)) = interaction_areas.get(child) {
                        let rect = Rect::from_center_size(
                            base_transform.translation().xy() * base_node.inverse_scale_factor,
                            base_node.size() * base_node.inverse_scale_factor,
                        );
                        interaction_rect = Some(rect);
                        break;
                    }
                    // Then try base background with ComputedNode (if available)
                    if let Ok((base_node, base_transform)) = base_backgrounds_query.get(child) {
                        let rect = Rect::from_center_size(
                            base_transform.translation().xy() * base_node.inverse_scale_factor,
                            base_node.size() * base_node.inverse_scale_factor,
                        );
                        interaction_rect = Some(rect);
                        break;
                    }
                }
            }
            
            // If ComputedNode not available, try calculating from Node style for base background
            // This works for fixed joysticks where base is positioned absolutely within parent
            // Skip this for floating joysticks (full-screen parents) - they should use parent rect
            if interaction_rect.is_none() {
                // Check if parent is full-screen (floating joystick case)
                let is_fullscreen_parent = if let Ok(parent_node) = parent_nodes.get(joystick_entity) {
                    matches!((parent_node.width, parent_node.height, parent_node.left, parent_node.bottom), 
                        (Val::Percent(100.0), Val::Percent(100.0), Val::Percent(0.0), Val::Percent(0.0)))
                } else {
                    false
                };
                
                if !is_fullscreen_parent {
                    // Only calculate base rect for non-fullscreen parents (fixed joysticks)
                    if let Ok(children) = children_query.get(joystick_entity) {
                        for &child in children.iter() {
                            if let Ok(base_node) = base_background_nodes.get(child) {
                                // Get base size from Node style
                                let base_width = match base_node.width {
                                    Val::Px(w) => w,
                                    _ => 150.0, // Default
                                };
                                let base_height = match base_node.height {
                                    Val::Px(h) => h,
                                    _ => 150.0, // Default
                                };
                                let base_size = Vec2::new(base_width, base_height);
                                
                                // For fixed joysticks, base is positioned at (0, 0) relative to parent (top-left)
                                // Calculate parent's actual screen position from Node style (left/right/bottom)
                                let parent_size = joystick_node.size() * joystick_node.inverse_scale_factor;
                                let mut parent_center_screen = joystick_global_transform.translation().xy() * joystick_node.inverse_scale_factor;
                                
                                // If GlobalTransform shows (0,0), calculate from Node style positioning
                                if parent_center_screen.length() < 0.1 {
                                    if let Ok(parent_node) = parent_nodes.get(joystick_entity) {
                                        if let Ok(window) = q_windows.single() {
                                            let window_width = window.width();
                                            let window_height = window.height();
                                            let window_center_x = window_width * 0.5;
                                            let window_center_y = window_height * 0.5;
                                            
                                            // Calculate parent center in window coordinates from Node style
                                            let parent_center_x_window = match parent_node.left {
                                                Val::Px(p) => p + parent_size.x * 0.5,
                                                Val::Percent(p) => (window_width * p / 100.0) + parent_size.x * 0.5,
                                                Val::Auto => match parent_node.right {
                                                    Val::Px(r) => window_width - r - parent_size.x * 0.5,
                                                    Val::Percent(r) => window_width - (window_width * r / 100.0) - parent_size.x * 0.5,
                                                    _ => window_center_x,
                                                },
                                                _ => window_center_x,
                                            };
                                            let parent_center_y_window = match parent_node.bottom {
                                                Val::Px(p) => window_height - p - parent_size.y * 0.5,
                                                Val::Percent(p) => window_height - (window_height * p / 100.0) - parent_size.y * 0.5,
                                                Val::Auto => match parent_node.top {
                                                    Val::Px(t) => t + parent_size.y * 0.5,
                                                    Val::Percent(t) => (window_height * t / 100.0) + parent_size.y * 0.5,
                                                    _ => window_center_y,
                                                },
                                                _ => window_center_y,
                                            };
                                            
                                            // Convert to screen coordinates
                                            parent_center_screen = Vec2::new(
                                                parent_center_x_window - window_center_x,
                                                parent_center_y_window - window_center_y,
                                            );
                                        }
                                    }
                                }
                                
                                // Parent top-left in screen coords
                                let parent_top_left_screen = parent_center_screen - parent_size * 0.5;
                                
                                // Base is at (0, 0) relative to parent, so base top-left = parent top-left
                                // Base center = base top-left + base_size * 0.5
                                let base_center_screen = parent_top_left_screen + base_size * 0.5;
                                
                                let rect = Rect::from_center_size(base_center_screen, base_size);
                                interaction_rect = Some(rect);
                                break;
                            }
                        }
                    }
                }
            }
            
            // If no base rect found, use parent rect (for floating joysticks)
            let interaction_rect = interaction_rect.unwrap_or_else(|| {
                Rect::from_center_size(
                    joystick_global_transform.translation().xy() * joystick_node.inverse_scale_factor,
                    joystick_node.size() * joystick_node.inverse_scale_factor,
                )
            });
            
            // Convert touch positions to screen coordinates (same as mouse)
            if let Ok(window) = q_windows.single() {
                let window_size = Vec2::new(window.width(), window.height());
                for touch in touches.iter() {
                    let touch_pos_window = touch.position();
                    let touch_pos_screen = touch_pos_window - window_size * 0.5;
                    if interaction_rect.contains(touch_pos_screen) {
                        joystick_state.touch_state = Some(TouchState {
                            id: touch.id(),
                            is_mouse: false,
                            start: touch_pos_screen,
                            current: touch_pos_screen,
                            just_pressed: true,
                        });
                        break;
                    }
                }
            }
            if joystick_state.touch_state.is_none() && mouse_buttons.pressed(MouseButton::Left)
            {
                if let Ok(window) = q_windows.single() {
                    if let Some(mouse_pos_window) = window.cursor_position() {
                        // Convert window coordinates (top-left origin) to screen coordinates (center origin)
                        let window_size = Vec2::new(window.width(), window.height());
                        let mouse_pos_screen = mouse_pos_window - window_size * 0.5;
                        if interaction_rect.contains(mouse_pos_screen) {
                            joystick_state.touch_state = Some(TouchState {
                                id: 0,
                                is_mouse: true,
                                start: mouse_pos_screen,
                                current: mouse_pos_screen,
                                just_pressed: mouse_buttons.just_pressed(MouseButton::Left),
                            });
                        }
                    }
                }
            }
        } else {
            let mut clear_touch_state = false;
            if let Some(touch_state) = &joystick_state.touch_state {
                if touch_state.is_mouse {
                    if mouse_buttons.just_released(MouseButton::Left) {
                        clear_touch_state = true;
                    }
                } else if touches.just_released(touch_state.id) {
                    clear_touch_state = true;
                }
            }
            if clear_touch_state {
                joystick_state.touch_state = None;
                joystick_state.just_released = true;
            } else if let Some(touch_state) = &mut joystick_state.touch_state {
                if touch_state.is_mouse {
                    if let Ok(window) = q_windows.single() {
                        if let Some(new_current_window) = window.cursor_position() {
                            // Convert window coordinates to screen coordinates
                            let window_size = Vec2::new(window.width(), window.height());
                            let new_current = new_current_window - window_size * 0.5;
                            if new_current != touch_state.current {
                                touch_state.current = new_current;
                            }
                        }
                    }
                } else if let Some(touch) = touches.get_pressed(touch_state.id) {
                    if let Ok(window) = q_windows.single() {
                        let window_size = Vec2::new(window.width(), window.height());
                        let touch_pos_window = touch.position();
                        let touch_pos_screen = touch_pos_window - window_size * 0.5;
                        if touch_pos_screen != touch_state.current {
                            touch_state.current = touch_pos_screen;
                        }
                    }
                }
            }
        }
    }
}

pub fn update_behavior_knob_delta<S: VirtualJoystickID>(world: &mut World) {
    let mut joysticks = world.query::<(Entity, &VirtualJoystickNode<S>)>();
    let mut joystick_entities: Vec<Entity> = Vec::new();
    for (joystick_entity, _) in joysticks.iter(world) {
        joystick_entities.push(joystick_entity);
    }
    for joystick_entity in joystick_entities {
        let behavior;
        {
            let Some(virtual_joystick_node) = world.get::<VirtualJoystickNode<S>>(joystick_entity)
            else {
                continue;
            };
            behavior = Arc::clone(&virtual_joystick_node.behavior);
        }
        behavior.update_at_delta_stage(world, joystick_entity);
    }
}

pub fn update_behavior_constraints<S: VirtualJoystickID>(world: &mut World) {
    let mut joysticks = world.query::<(Entity, &VirtualJoystickNode<S>)>();
    let mut joystick_entities: Vec<Entity> = Vec::new();
    for (joystick_entity, _) in joysticks.iter(world) {
        joystick_entities.push(joystick_entity);
    }
    for joystick_entity in joystick_entities {
        let behavior;
        {
            let Some(virtual_joystick_node) = world.get::<VirtualJoystickNode<S>>(joystick_entity)
            else {
                continue;
            };
            behavior = Arc::clone(&virtual_joystick_node.behavior);
        }
        behavior.update_at_constraint_stage(world, joystick_entity);
    }
}

pub fn update_behavior<S: VirtualJoystickID>(world: &mut World) {
    let mut joysticks = world.query::<(Entity, &VirtualJoystickNode<S>)>();
    let mut joystick_entities: Vec<Entity> = Vec::new();
    for (joystick_entity, _) in joysticks.iter(world) {
        joystick_entities.push(joystick_entity);
    }
    for joystick_entity in joystick_entities {
        let behavior;
        {
            let Some(virtual_joystick_node) = world.get::<VirtualJoystickNode<S>>(joystick_entity)
            else {
                continue;
            };
            behavior = Arc::clone(&virtual_joystick_node.behavior);
        }
        behavior.update(world, joystick_entity);
    }
}

pub fn update_action<S: VirtualJoystickID>(world: &mut World) {
    let mut joysticks =
        world.query::<(Entity, &VirtualJoystickNode<S>, &mut VirtualJoystickState)>();
    let mut joystick_entities: Vec<Entity> = Vec::new();
    for (joystick_entity, _, _) in joysticks.iter(world) {
        joystick_entities.push(joystick_entity);
    }
    enum DragAction {
        StartDrag,
        Drag,
        EndDrag,
    }
    for joystick_entity in joystick_entities {
        let drag_action: Option<DragAction>;
        {
            let Some(joystick_state) = world.get::<VirtualJoystickState>(joystick_entity) else {
                continue;
            };
            if joystick_state.just_released {
                drag_action = Some(DragAction::EndDrag);
            } else if let Some(touch_state) = &joystick_state.touch_state {
                if touch_state.just_pressed {
                    drag_action = Some(DragAction::StartDrag);
                } else {
                    drag_action = Some(DragAction::Drag);
                }
            } else {
                drag_action = None;
            }
        }
        let Some(drag_action) = drag_action else {
            continue;
        };
        let id;
        let action;
        let joystick_state;
        {
            let Ok((_, virtual_joystick_node, joystick_state_2)) =
                joysticks.get_mut(world, joystick_entity)
            else {
                continue;
            };
            id = virtual_joystick_node.id.clone();
            action = Arc::clone(&virtual_joystick_node.action);
            joystick_state = joystick_state_2.clone();
        }
        match drag_action {
            DragAction::StartDrag => {
                action.on_start_drag(id, joystick_state, world, joystick_entity);
            }
            DragAction::Drag => {
                action.on_drag(id, joystick_state, world, joystick_entity);
            }
            DragAction::EndDrag => {
                action.on_end_drag(id, joystick_state, world, joystick_entity);
            }
        }
    }
}

pub fn update_fire_events<S: VirtualJoystickID>(
    joysticks: Query<(&VirtualJoystickNode<S>, &VirtualJoystickState)>,
    mut send_values: MessageWriter<VirtualJoystickEvent<S>>,
) {
    for (joystick, joystick_state) in &joysticks {
        if joystick_state.just_released {
            send_values.write(VirtualJoystickEvent {
                id: joystick.id.clone(),
                event: VirtualJoystickEventType::Up,
                value: Vec2::ZERO,
                delta: joystick_state.delta,
            });
            continue;
        }
        if let Some(touch_state) = &joystick_state.touch_state {
            if touch_state.just_pressed {
                send_values.write(VirtualJoystickEvent {
                    id: joystick.id.clone(),
                    event: VirtualJoystickEventType::Press,
                    value: touch_state.current,
                    delta: joystick_state.delta,
                });
            }
            send_values.write(VirtualJoystickEvent {
                id: joystick.id.clone(),
                event: VirtualJoystickEventType::Drag,
                value: touch_state.current,
                delta: joystick_state.delta,
            });
        }
    }
}

#[allow(clippy::complexity)]
pub fn update_ui(
    joysticks: Query<(&VirtualJoystickState, &Children, &ComputedNode, &GlobalTransform, Option<&Visibility>)>,
    mut joystick_bases: Query<&mut Node, With<VirtualJoystickUIBackground>>,
    mut joystick_knobs: Query<
        &mut Node,
        (
            With<VirtualJoystickUIKnob>,
            Without<VirtualJoystickUIBackground>,
        ),
    >,
    base_nodes: Query<&ComputedNode, With<VirtualJoystickUIBackground>>,
    knob_nodes: Query<&ComputedNode, (With<VirtualJoystickUIKnob>, Without<VirtualJoystickUIBackground>)>,
) {
    for (joystick_state, children, joystick_node, joystick_global_transform, visibility) in &joysticks {
        // Skip positioning if joystick is hidden (but still update if it's visible or inherited)
        // However, if there's a touch_state, we should position even if visibility check fails
        // (this handles the case where joystick just became visible but ComputedNode isn't ready yet)
        let is_hidden = visibility.map(|v| *v == Visibility::Hidden).unwrap_or(false);
        if is_hidden && joystick_state.touch_state.is_none() {
            continue;
        }
        let mut joystick_base_rect: Option<Rect> = None;
        // Get parent size in screen coordinates
        let parent_size = joystick_node.size() * joystick_node.inverse_scale_factor;
        let parent_center_screen = joystick_global_transform.translation().xy() * joystick_node.inverse_scale_factor;
        
        for child in children.iter() {
            if let Ok(mut joystick_base_style) = joystick_bases.get_mut(*child) {
                // Get base size - try ComputedNode first, fall back to Node
                let base_size = if let Ok(base_node) = base_nodes.get(*child) {
                    base_node.size() * base_node.inverse_scale_factor
                } else {
                    // Fallback: get from the Node style if ComputedNode not available
                    let width = match joystick_base_style.width {
                        Val::Px(w) => w,
                        _ => 150.0,
                    };
                    let height = match joystick_base_style.height {
                        Val::Px(h) => h,
                        _ => 150.0,
                    };
                    Vec2::new(width, height)
                };
                
                joystick_base_style.position_type = PositionType::Absolute;
                // base_offset is in screen coordinates (offset from parent center to touch position)
                // Parent center is at parent_center_screen in screen coords
                // Parent top-left in screen coords = parent_center_screen - parent_size * 0.5
                // To convert screen coord to parent-relative: screen_coord - parent_top_left
                // Base center in screen coords = parent_center_screen + base_offset
                // Base center in parent-relative = (parent_center_screen + base_offset) - (parent_center_screen - parent_size * 0.5)
                //                                 = base_offset + parent_size * 0.5
                let base_center_in_parent = joystick_state.base_offset + parent_size * 0.5;
                let base_left = base_center_in_parent.x - base_size.x * 0.5;
                let base_top = base_center_in_parent.y - base_size.y * 0.5;
                joystick_base_style.left = Val::Px(base_left);
                joystick_base_style.top = Val::Px(base_top);

                // Calculate base rect for knob positioning
                let base_center_screen = parent_center_screen + joystick_state.base_offset;
                let rect = Rect::from_center_size(base_center_screen, base_size);
                joystick_base_rect = Some(rect);
            }
        }
        if joystick_base_rect.is_none() {
            continue;
        }
        let joystick_base_rect = joystick_base_rect.unwrap();
        let joystick_base_rect_half_size = joystick_base_rect.half_size();
        for child in children.iter() {
            if let Ok(mut joystick_knob_style) = joystick_knobs.get_mut(*child) {
                // Get knob size - try ComputedNode first, fall back to Node
                let knob_size = if let Ok(knob_node) = knob_nodes.get(*child) {
                    knob_node.size() * knob_node.inverse_scale_factor
                } else {
                    // Fallback: get from the Node style if ComputedNode not available
                    let width = match joystick_knob_style.width {
                        Val::Px(w) => w,
                        _ => 75.0,
                    };
                    let height = match joystick_knob_style.height {
                        Val::Px(h) => h,
                        _ => 75.0,
                    };
                    Vec2::new(width, height)
                };
                let joystick_knob_half_size = knob_size * 0.5;
                
                joystick_knob_style.position_type = PositionType::Absolute;
                // Position knob center relative to base center
                // Base center in parent coords = base_offset + parent_size/2
                let base_center_in_parent = joystick_state.base_offset + parent_size * 0.5;
                let knob_center_in_parent = base_center_in_parent + Vec2::new(
                    joystick_state.delta.x * joystick_base_rect_half_size.x,
                    -joystick_state.delta.y * joystick_base_rect_half_size.y,
                );
                let knob_left = knob_center_in_parent.x - joystick_knob_half_size.x;
                let knob_top = knob_center_in_parent.y - joystick_knob_half_size.y;
                joystick_knob_style.left = Val::Px(knob_left);
                joystick_knob_style.top = Val::Px(knob_top);
            }
        }
    }
}
