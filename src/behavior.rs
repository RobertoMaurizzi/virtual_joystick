use std::sync::Arc;

use bevy::{
    camera::visibility::Visibility,
    ecs::{entity::Entity, query::With, world::World},
    math::{Rect, Vec2, Vec3Swizzles},
    prelude::Children,
    reflect::Reflect,
    transform::components::GlobalTransform,
    ui::{ComputedNode, Node, Val},
    window::{PrimaryWindow, Window},
};
use variadics_please::all_tuples;

use crate::{components::VirtualJoystickState, VirtualJoystickUIBackground};

pub trait VirtualJoystickBehavior: Send + Sync + 'static {
    fn update_at_delta_stage(&self, _world: &mut World, _entity: Entity) {}
    fn update_at_constraint_stage(&self, _world: &mut World, _entity: Entity) {}
    fn update(&self, _world: &mut World, _entity: Entity) {}
}

impl<A: VirtualJoystickBehavior + Clone> VirtualJoystickBehavior for Arc<A> {
    fn update_at_delta_stage(&self, world: &mut World, entity: Entity) {
        (**self).update_at_delta_stage(world, entity);
    }
    fn update_at_constraint_stage(&self, world: &mut World, entity: Entity) {
        (**self).update_at_constraint_stage(world, entity);
    }
    fn update(&self, world: &mut World, entity: Entity) {
        (**self).update(world, entity);
    }
}

macro_rules! impl_behavior_sets {
    ($($set: ident),*) => {
        impl<$($set: VirtualJoystickBehavior),*> VirtualJoystickBehavior for ($($set,)*)
        {
            #[allow(non_snake_case)]
            fn update_at_delta_stage(&self, world: &mut World, entity: Entity) {
                let ($($set,)*) = self;
                $($set.update_at_delta_stage(world, entity);)*
            }
            #[allow(non_snake_case)]
            fn update_at_constraint_stage(&self, world: &mut World, entity: Entity) {
                let ($($set,)*) = self;
                $($set.update_at_constraint_stage(world, entity);)*
            }
            #[allow(non_snake_case)]
            fn update(&self, world: &mut World, entity: Entity) {
                let ($($set,)*) = self;
                $($set.update(world, entity);)*
            }
        }
    }
}

all_tuples!(impl_behavior_sets, 1, 20, S);

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickDeadZone(pub f32);

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickDigital8;

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickHorizontalOnly;

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickVerticalOnly;

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickInvisible;

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickFixed;

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickFloating;

#[derive(Clone, Copy, Debug, Default, Reflect)]
pub struct JoystickDynamic;

impl VirtualJoystickBehavior for JoystickDeadZone {
    fn update_at_constraint_stage(&self, world: &mut World, entity: Entity) {
        let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
            return;
        };
        let dead_zone = self.0;
        if joystick_state.delta.x.abs() < dead_zone {
            joystick_state.delta.x = 0.0;
        }
        if joystick_state.delta.y.abs() < dead_zone {
            joystick_state.delta.y = 0.0;
        }
    }
}

impl VirtualJoystickBehavior for JoystickDigital8 {
    fn update_at_constraint_stage(&self, world: &mut World, entity: Entity) {
        let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
            return;
        };

        let delta = joystick_state.delta;
        let magnitude = delta.length();

        // If the magnitude is very small, snap to zero (center/neutral)
        if magnitude < 0.1 {
            joystick_state.delta = Vec2::ZERO;
            return;
        }

        // Calculate angle in radians using atan2
        // In the delta coordinate system: x = right/left, y = up/down (after inversion in update_at_delta_stage)
        // atan2(y, x) gives: 0 = right (E), π/2 = up (N), π = left (W), -π/2 = down (S)
        let angle = delta.y.atan2(delta.x);

        // Snap to nearest 45-degree increment (8 directions: N, NE, E, SE, S, SW, W, NW)
        // Divide by π/4 (45 degrees) and round to nearest integer
        let snapped_angle =
            (angle / (std::f32::consts::PI / 4.0)).round() * (std::f32::consts::PI / 4.0);

        // Convert back to direction vector
        // Using cos for x and sin for y (standard atan2 convention)
        let snapped_delta = Vec2::new(snapped_angle.cos(), snapped_angle.sin());

        // Normalize to unit length for true digital behavior (always full strength)
        joystick_state.delta = snapped_delta.normalize_or_zero();
    }
}

impl VirtualJoystickBehavior for JoystickHorizontalOnly {
    fn update_at_constraint_stage(&self, world: &mut World, entity: Entity) {
        let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
            return;
        };
        joystick_state.delta.y = 0.0;
    }
}

impl VirtualJoystickBehavior for JoystickVerticalOnly {
    fn update_at_constraint_stage(&self, world: &mut World, entity: Entity) {
        let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
            return;
        };
        joystick_state.delta.x = 0.0;
    }
}

impl VirtualJoystickBehavior for JoystickInvisible {
    fn update(&self, world: &mut World, entity: Entity) {
        let joystick_state = world.get::<VirtualJoystickState>(entity).cloned();
        let Some(joystick_state) = joystick_state else {
            return;
        };
        let Some(mut joystick_visibility) = world.get_mut::<Visibility>(entity) else {
            return;
        };
        if joystick_state.just_released
            || *joystick_visibility != Visibility::Hidden && joystick_state.touch_state.is_none()
        {
            *joystick_visibility = Visibility::Hidden;
        }
        if let Some(touch_state) = &joystick_state.touch_state {
            if touch_state.just_pressed {
                *joystick_visibility = Visibility::Inherited;
            }
        }
    }
}

// impl VirtualJoystickBehavior for JoystickInvisible {
//     fn update(&self, world: &mut World, entity: Entity) {
//         let joystick_state = world.get::<VirtualJoystickState>(entity).cloned();
//         let Some(joystick_state) = joystick_state else {
//             return;
//         };
//         let Some(mut joystick_visibility) = world.get_mut::<Visibility>(entity) else {
//             return;
//         };
//
//         // Determine if joystick should be visible based on touch state
//         let should_be_visible = joystick_state.touch_state.is_some();
//
//         if should_be_visible {
//             // Show joystick when touched
//             if *joystick_visibility == Visibility::Hidden {
//                 *joystick_visibility = Visibility::Inherited;
//             }
//         } else {
//             // Hide joystick when not touched (including on release and initially)
//             if *joystick_visibility != Visibility::Hidden {
//                 *joystick_visibility = Visibility::Hidden;
//             }
//         }
//
//         // Also update children's visibility to ensure they're hidden/shown too
//         if let Some(children) = world.get::<Children>(entity) {
//             let child_entities: Vec<Entity> = children.iter().copied().collect();
//             for child in child_entities {
//                 if let Some(mut child_visibility) = world.get_mut::<Visibility>(child) {
//                     if should_be_visible {
//                         if *child_visibility == Visibility::Hidden {
//                             *child_visibility = Visibility::Inherited;
//                         }
//                     } else {
//                         if *child_visibility != Visibility::Hidden {
//                             *child_visibility = Visibility::Hidden;
//                         }
//                     }
//                 }
//             }
//         }
//     }
// }
//

impl VirtualJoystickBehavior for JoystickFixed {
    fn update_at_delta_stage(&self, world: &mut World, entity: Entity) {
        // Get parent (joystick entity) transform and size
        let Some(joystick_node) = world.get::<ComputedNode>(entity) else {
            return;
        };
        let Some(joystick_global_transform) = world.get::<GlobalTransform>(entity) else {
            return;
        };

        let parent_size = joystick_node.size() * joystick_node.inverse_scale_factor;
        let mut parent_center_screen =
            joystick_global_transform.translation().xy() * joystick_node.inverse_scale_factor;

        // If GlobalTransform shows (0,0), calculate from Node style positioning
        if parent_center_screen.length() < 0.1 {
            let parent_node_opt = world.get::<Node>(entity).cloned();
            if let Some(parent_node) = parent_node_opt {
                // Try to get window from world using query_filtered
                let mut window_query = world.query_filtered::<&Window, With<PrimaryWindow>>();
                if let Some(window) = window_query.iter(world).next() {
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
                            Val::Percent(r) => {
                                window_width - (window_width * r / 100.0) - parent_size.x * 0.5
                            }
                            _ => window_center_x,
                        },
                        _ => window_center_x,
                    };
                    let parent_center_y_window = match parent_node.bottom {
                        Val::Px(p) => window_height - p - parent_size.y * 0.5,
                        Val::Percent(p) => {
                            window_height - (window_height * p / 100.0) - parent_size.y * 0.5
                        }
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

        // Get base size from child's Node style (ComputedNode might not be available in PreUpdate)
        let mut base_size = Vec2::new(150.0, 150.0); // Default
        let Some(children) = world.get::<Children>(entity) else {
            return;
        };

        for &child in children.iter() {
            if world.get::<VirtualJoystickUIBackground>(child).is_none() {
                continue;
            }
            // Try ComputedNode first (might be available in some cases)
            if let Some(computed_node) = world.get::<ComputedNode>(child) {
                base_size = computed_node.size() * computed_node.inverse_scale_factor;
            } else if let Some(node) = world.get::<Node>(child) {
                // Fallback to Node style
                let width = match node.width {
                    bevy::ui::Val::Px(w) => w,
                    _ => 150.0,
                };
                let height = match node.height {
                    bevy::ui::Val::Px(h) => h,
                    _ => 150.0,
                };
                base_size = Vec2::new(width, height);
            }
            break;
        }

        // For fixed joysticks, base is positioned at (0, 0) relative to parent (top-left)
        // Parent top-left in screen coords = parent_center_screen - parent_size * 0.5
        // Base center in screen coords = parent_top_left + base_size * 0.5
        let parent_top_left_screen = parent_center_screen - parent_size * 0.5;
        let base_center_screen = parent_top_left_screen + base_size * 0.5;
        let joystick_base_rect = Rect::from_center_size(base_center_screen, base_size);

        let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
            return;
        };
        joystick_state.base_offset = Vec2::ZERO;
        let new_delta: Vec2;
        if let Some(touch_state) = &joystick_state.touch_state {
            let mut offset = touch_state.current - joystick_base_rect.center();

            let max_distance = joystick_base_rect.half_size().x;
            let distance_squared = offset.length_squared();

            if distance_squared > max_distance * max_distance {
                let distance = distance_squared.sqrt();
                offset = offset * (max_distance / distance);
            }

            let mut new_delta2 = (offset / joystick_base_rect.half_size())
                .clamp(Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0));
            new_delta2.y = -new_delta2.y;
            new_delta = new_delta2;
        } else {
            new_delta = Vec2::ZERO;
        }
        joystick_state.delta = new_delta;
    }
}

impl VirtualJoystickBehavior for JoystickFloating {
    fn update_at_delta_stage(&self, world: &mut World, entity: Entity) {
        // Check if joystick has touch_state - if so, we need to process even if ComputedNode isn't ready yet
        // (this handles the case where joystick is hidden and just became visible)
        // FIXME: validate if this is really necessary
        let joystick_state_ref = world.get::<VirtualJoystickState>(entity);
        let has_touch_state = joystick_state_ref
            .and_then(|state| state.touch_state.as_ref())
            .is_some();

        // Get parent (joystick entity) transform and size
        let Some(joystick_node) = world.get::<ComputedNode>(entity) else {
            return;
        };
        let Some(joystick_global_transform) = world.get::<GlobalTransform>(entity) else {
            return;
        };

        // Get base size from child
        let mut base_size = Vec2::new(150.0, 150.0); // Default
        let Some(children) = world.get::<Children>(entity) else {
            return;
        };

        for &child in children.iter() {
            if world.get::<VirtualJoystickUIBackground>(child).is_some() {
                // Try to get size from ComputedNode or Node
                if let Some(computed_node) = world.get::<ComputedNode>(child) {
                    base_size = computed_node.size() * computed_node.inverse_scale_factor;
                } else if let Some(node) = world.get::<Node>(child) {
                    let width = match node.width {
                        bevy::ui::Val::Px(w) => w,
                        _ => 150.0,
                    };
                    let height = match node.height {
                        bevy::ui::Val::Px(h) => h,
                        _ => 150.0,
                    };
                    base_size = Vec2::new(width, height);
                }
                break;
            }
        }

        let parent_size = joystick_node.size() * joystick_node.inverse_scale_factor;
        let mut parent_center_screen =
            joystick_global_transform.translation().xy() * joystick_node.inverse_scale_factor;

        // If GlobalTransform shows (0,0), calculate from Node style positioning
        if parent_center_screen.length() < 0.1 {
            let parent_node_opt = world.get::<Node>(entity).cloned();
            if let Some(parent_node) = parent_node_opt {
                // Try to get window from world using query_filtered
                let mut window_query = world.query_filtered::<&Window, With<PrimaryWindow>>();
                if let Some(window) = window_query.iter(world).next() {
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
                            Val::Percent(r) => {
                                window_width - (window_width * r / 100.0) - parent_size.x * 0.5
                            }
                            _ => window_center_x,
                        },
                        _ => window_center_x,
                    };
                    let parent_center_y_window = match parent_node.bottom {
                        Val::Px(p) => window_height - p - parent_size.y * 0.5,
                        Val::Percent(p) => {
                            window_height - (window_height * p / 100.0) - parent_size.y * 0.5
                        }
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

        let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
            return;
        };

        let base_offset: Vec2;
        let mut assign_base_offset = false;
        let mut is_just_pressed = false;

        if let Some(touch_state) = &joystick_state.touch_state {
            if touch_state.just_pressed {
                // Calculate base_offset: offset from parent center to touch start
                base_offset = touch_state.start - parent_center_screen;
                assign_base_offset = true;
                is_just_pressed = true;
            } else {
                base_offset = joystick_state.base_offset;
            }
        } else if joystick_state.just_released {
            // For floating joystick, keep base_offset where it is (don't reset to zero)
            // Only reset if explicitly needed
            base_offset = joystick_state.base_offset;
            // Don't assign - keep it where it was
        } else {
            base_offset = joystick_state.base_offset;
        }

        if assign_base_offset {
            joystick_state.base_offset = base_offset;
        }

        // Calculate base center from parent center + base_offset
        let base_center_screen = parent_center_screen + base_offset;
        let base_half_size = base_size * 0.5;
        // let joystick_base_rect = Rect::from_center_size(base_center_screen, base_size);

        let new_delta: Vec2;

        if is_just_pressed {
            new_delta = Vec2::ZERO;
        } else if let Some(touch_state) = &joystick_state.touch_state {
            let mut offset = touch_state.current - base_center_screen;
            let max_distance = base_half_size.x;
            let distance_squared = offset.length_squared();

            if distance_squared > max_distance * max_distance {
                let distance = distance_squared.sqrt();
                offset *= max_distance / distance;
            }

            let mut new_delta2 =
                (offset / max_distance).clamp(Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0));
            new_delta2.y = -new_delta2.y;
            new_delta = new_delta2;
        } else {
            new_delta = Vec2::ZERO;
        }

        joystick_state.delta = new_delta;
    }
}

// impl VirtualJoystickBehavior for JoystickFloating {
//     fn update_at_delta_stage(&self, world: &mut World, entity: Entity) {
//         // Check if joystick has touch_state - if so, we need to process even if ComputedNode isn't ready yet
//         // (this handles the case where joystick is hidden and just became visible)
//         let joystick_state_ref = world.get::<VirtualJoystickState>(entity);
//         let has_touch_state = joystick_state_ref
//             .and_then(|state| state.touch_state.as_ref())
//             .is_some();
//
//         // Get parent (joystick entity) transform and size
//         // Try ComputedNode first, but if not available and we have touch_state, calculate from Node style
//         let (parent_size, mut parent_center_screen) =
//             if let (Some(joystick_node), Some(joystick_global_transform)) = (
//                 world.get::<ComputedNode>(entity),
//                 world.get::<GlobalTransform>(entity),
//             ) {
//                 let size = joystick_node.size() * joystick_node.inverse_scale_factor;
//                 let center = joystick_global_transform.translation().xy()
//                     * joystick_node.inverse_scale_factor;
//                 (size, center)
//             } else if has_touch_state {
//                 // Fallback: calculate from Node style when ComputedNode not available
//                 let parent_node_opt = world.get::<Node>(entity).cloned();
//                 if let Some(parent_node) = parent_node_opt {
//                     // Get parent size from Node style
//                     let parent_width = match parent_node.width {
//                         Val::Px(w) => w,
//                         Val::Percent(p) => {
//                             // Need window size for percentage - try to get it
//                             let mut window_query =
//                                 world.query_filtered::<&Window, With<PrimaryWindow>>();
//                             if let Some(window) = window_query.iter(world).next() {
//                                 window.width() * p / 100.0
//                             } else {
//                                 1280.0 * p / 100.0 // Default fallback
//                             }
//                         }
//                         _ => 1280.0, // Default fallback
//                     };
//                     let parent_height = match parent_node.height {
//                         Val::Px(h) => h,
//                         Val::Percent(p) => {
//                             let mut window_query =
//                                 world.query_filtered::<&Window, With<PrimaryWindow>>();
//                             if let Some(window) = window_query.iter(world).next() {
//                                 window.height() * p / 100.0
//                             } else {
//                                 720.0 * p / 100.0 // Default fallback
//                             }
//                         }
//                         _ => 720.0, // Default fallback
//                     };
//                     let size = Vec2::new(parent_width, parent_height);
//
//                     // Calculate parent center from Node style
//                     let mut window_query = world.query_filtered::<&Window, With<PrimaryWindow>>();
//                     if let Some(window) = window_query.iter(world).next() {
//                         let window_width = window.width();
//                         let window_height = window.height();
//                         let window_center_x = window_width * 0.5;
//                         let window_center_y = window_height * 0.5;
//
//                         let parent_center_x_window = match parent_node.left {
//                             Val::Px(p) => p + size.x * 0.5,
//                             Val::Percent(p) => (window_width * p / 100.0) + size.x * 0.5,
//                             Val::Auto => match parent_node.right {
//                                 Val::Px(r) => window_width - r - size.x * 0.5,
//                                 Val::Percent(r) => {
//                                     window_width - (window_width * r / 100.0) - size.x * 0.5
//                                 }
//                                 _ => window_center_x,
//                             },
//                             _ => window_center_x,
//                         };
//                         let parent_center_y_window = match parent_node.bottom {
//                             Val::Px(p) => window_height - p - size.y * 0.5,
//                             Val::Percent(p) => {
//                                 window_height - (window_height * p / 100.0) - size.y * 0.5
//                             }
//                             Val::Auto => match parent_node.top {
//                                 Val::Px(t) => t + size.y * 0.5,
//                                 Val::Percent(t) => (window_height * t / 100.0) + size.y * 0.5,
//                                 _ => window_center_y,
//                             },
//                             _ => window_center_y,
//                         };
//
//                         let center = Vec2::new(
//                             parent_center_x_window - window_center_x,
//                             parent_center_y_window - window_center_y,
//                         );
//                         (size, center)
//                     } else {
//                         // No window available, use defaults
//                         (size, Vec2::ZERO)
//                     }
//                 } else {
//                     return; // Can't calculate without Node
//                 }
//             } else {
//                 return; // No ComputedNode and no touch_state
//             };
//
//         // Get base size from child
//         let mut base_size = Vec2::new(150.0, 150.0); // Default
//         let Some(children) = world.get::<Children>(entity) else {
//             return;
//         };
//
//         for &child in children.iter() {
//             if world.get::<VirtualJoystickUIBackground>(child).is_some() {
//                 // Try to get size from ComputedNode or Node
//                 if let Some(computed_node) = world.get::<ComputedNode>(child) {
//                     base_size = computed_node.size() * computed_node.inverse_scale_factor;
//                 } else if let Some(node) = world.get::<Node>(child) {
//                     let width = match node.width {
//                         bevy::ui::Val::Px(w) => w,
//                         _ => 150.0,
//                     };
//                     let height = match node.height {
//                         bevy::ui::Val::Px(h) => h,
//                         _ => 150.0,
//                     };
//                     base_size = Vec2::new(width, height);
//                 }
//                 break;
//             }
//         }
//
//         // If parent_center_screen is still (0,0) and we got ComputedNode, try to recalculate from Node style
//         // (This is a fallback for when GlobalTransform is at (0,0) even though ComputedNode exists)
//         if parent_center_screen.length() < 0.1 && world.get::<ComputedNode>(entity).is_some() {
//             let parent_node_opt = world.get::<Node>(entity).cloned();
//             if let Some(parent_node) = parent_node_opt {
//                 // Try to get window from world using query_filtered
//                 let mut window_query = world.query_filtered::<&Window, With<PrimaryWindow>>();
//                 if let Some(window) = window_query.iter(world).next() {
//                     let window_width = window.width();
//                     let window_height = window.height();
//                     let window_center_x = window_width * 0.5;
//                     let window_center_y = window_height * 0.5;
//
//                     // Calculate parent center in window coordinates from Node style
//                     let parent_center_x_window = match parent_node.left {
//                         Val::Px(p) => p + parent_size.x * 0.5,
//                         Val::Percent(p) => (window_width * p / 100.0) + parent_size.x * 0.5,
//                         Val::Auto => match parent_node.right {
//                             Val::Px(r) => window_width - r - parent_size.x * 0.5,
//                             Val::Percent(r) => {
//                                 window_width - (window_width * r / 100.0) - parent_size.x * 0.5
//                             }
//                             _ => window_center_x,
//                         },
//                         _ => window_center_x,
//                     };
//                     let parent_center_y_window = match parent_node.bottom {
//                         Val::Px(p) => window_height - p - parent_size.y * 0.5,
//                         Val::Percent(p) => {
//                             window_height - (window_height * p / 100.0) - parent_size.y * 0.5
//                         }
//                         Val::Auto => match parent_node.top {
//                             Val::Px(t) => t + parent_size.y * 0.5,
//                             Val::Percent(t) => (window_height * t / 100.0) + parent_size.y * 0.5,
//                             _ => window_center_y,
//                         },
//                         _ => window_center_y,
//                     };
//
//                     // Convert to screen coordinates
//                     parent_center_screen = Vec2::new(
//                         parent_center_x_window - window_center_x,
//                         parent_center_y_window - window_center_y,
//                     );
//                 }
//             }
//         }
//
//         let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
//             return;
//         };
//
//         let base_offset: Vec2;
//         let mut assign_base_offset = false;
//         let mut is_just_pressed = false;
//
//         if let Some(touch_state) = &joystick_state.touch_state {
//             if touch_state.just_pressed {
//                 // Calculate base_offset: offset from parent center to touch start
//                 base_offset = touch_state.start - parent_center_screen;
//                 assign_base_offset = true;
//                 is_just_pressed = true;
//             } else {
//                 base_offset = joystick_state.base_offset;
//             }
//         } else if joystick_state.just_released {
//             // For floating joystick, keep base_offset where it is (don't reset to zero)
//             // Only reset if explicitly needed
//             base_offset = joystick_state.base_offset;
//             // Don't assign - keep it where it was
//         } else {
//             base_offset = joystick_state.base_offset;
//         }
//
//         if assign_base_offset {
//             joystick_state.base_offset = base_offset;
//         }
//
//         // Calculate base center from parent center + base_offset
//         let base_center_screen = parent_center_screen + base_offset;
//         let base_half_size = base_size * 0.5;
//         let joystick_base_rect = Rect::from_center_size(base_center_screen, base_size);
//
//         let new_delta: Vec2;
//
//         if is_just_pressed {
//             new_delta = Vec2::ZERO;
//         } else if let Some(touch_state) = &joystick_state.touch_state {
//             let mut offset = touch_state.current - base_center_screen;
//             let max_distance = base_half_size.x;
//             let distance_squared = offset.length_squared();
//
//             if distance_squared > max_distance * max_distance {
//                 let distance = distance_squared.sqrt();
//                 offset *= max_distance / distance;
//             }
//
//             let mut new_delta2 =
//                 (offset / max_distance).clamp(Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0));
//             new_delta2.y = -new_delta2.y;
//             new_delta = new_delta2;
//         } else {
//             new_delta = Vec2::ZERO;
//         }
//
//         joystick_state.delta = new_delta;
//     }
// }

impl VirtualJoystickBehavior for JoystickDynamic {
    fn update_at_delta_stage(&self, world: &mut World, entity: Entity) {
        let joystick_rect: Rect;
        {
            let Some(joystick_node) = world.get::<ComputedNode>(entity) else {
                return;
            };
            let Some(joystick_global_transform) = world.get::<GlobalTransform>(entity) else {
                return;
            };
            joystick_rect = Rect::from_center_size(
                joystick_global_transform.translation().xy(),
                joystick_node.size(),
            );
        }
        let mut joystick_base_rect: Option<Rect> = None;
        let Some(children) = world.get::<Children>(entity) else {
            return;
        };

        for &child in children.iter() {
            if world.get::<VirtualJoystickUIBackground>(child).is_none() {
                continue;
            }
            let Some(joystick_base_node) = world.get::<ComputedNode>(child) else {
                continue;
            };
            let Some(joystick_base_global_transform) = world.get::<GlobalTransform>(child) else {
                continue;
            };
            let rect = Rect::from_center_size(
                joystick_base_global_transform.translation().xy(),
                joystick_base_node.size(),
            );
            joystick_base_rect = Some(rect);
            break;
        }
        let Some(joystick_base_rect) = joystick_base_rect else {
            return;
        };
        let Some(mut joystick_state) = world.get_mut::<VirtualJoystickState>(entity) else {
            return;
        };
        let joystick_base_rect_center = joystick_base_rect.center();
        let joystick_base_rect_half_size = joystick_base_rect.half_size();
        let base_offset: Vec2;
        let mut assign_base_offset = false;
        if let Some(touch_state) = &joystick_state.touch_state {
            if touch_state.just_pressed {
                base_offset = touch_state.start - joystick_base_rect_center;
                assign_base_offset = true;
            } else {
                base_offset = joystick_state.base_offset;
            }
        } else if joystick_state.just_released {
            base_offset = Vec2::ZERO;
            assign_base_offset = true;
        } else {
            base_offset = joystick_state.base_offset;
        }
        if assign_base_offset {
            joystick_state.base_offset = base_offset;
        }
        let new_delta: Vec2;
        let mut new_base_offset: Option<Vec2> = None;
        if let Some(touch_state) = &joystick_state.touch_state {
            let mut offset = touch_state.current
                - (joystick_rect.min + base_offset + joystick_base_rect.half_size());

            let max_distance = joystick_base_rect_half_size.x;
            let distance_squared = offset.length_squared();

            if distance_squared > max_distance * max_distance {
                let distance = distance_squared.sqrt();
                offset = offset * (max_distance / distance);
                new_base_offset =
                    Some(base_offset + (offset - (offset * (max_distance / distance))));
            }

            let mut new_delta2 = (offset / joystick_base_rect_half_size)
                .clamp(Vec2::new(-1.0, -1.0), Vec2::new(1.0, 1.0));
            new_delta2.y = -new_delta2.y;
            new_delta = new_delta2;
        } else {
            new_delta = Vec2::ZERO;
        }
        joystick_state.delta = new_delta;
        if let Some(base_offset) = new_base_offset {
            joystick_state.base_offset = base_offset;
        }
    }
}
