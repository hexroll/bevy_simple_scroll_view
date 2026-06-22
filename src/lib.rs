#![doc = include_str!("../README.md")]

use core::f32;

use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
};

/// A `Plugin` providing the systems and components required to make a ScrollView work.
///
/// # Example
/// ```no_run
/// use bevy::prelude::*;
/// use bevy_simple_scroll_view::*;
///
/// App::new()
///     .add_plugins((DefaultPlugins,ScrollViewPlugin))
///     .run();
/// ```
pub struct ScrollViewPlugin;

impl Plugin for ScrollViewPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ScrollView>()
            .register_type::<ScrollableContent>()
            .register_type::<ScrollTarget>()
            .add_systems(
                Update,
                (
                    create_scroll_view,
                    update_size,
                    input_mouse_pressed_move,
                    input_touch_pressed_move,
                    scroll_events,
                    scroll_update,
                )
                    .chain(),
            );
    }
}

/// Root component of scroll, it should have clipped style.
#[derive(Component, Debug, Reflect)]
#[require(Interaction, Node = scroll_view_node())]
pub struct ScrollView {
    /// Field which control speed of the scrolling.
    /// Could be negative number to implement invert scroll
    pub scroll_speed: f32,
}

impl Default for ScrollView {
    fn default() -> Self {
        Self {
            scroll_speed: 1200.0,
        }
    }
}

/// Component containing offset value of the scroll container to the parent.
/// It is possible to update the field `pos_y` manually to move scrollview to desired location.
#[derive(Component, Debug, Reflect, Default)]
#[require(Node = scroll_content_node())]
pub struct ScrollableContent {
    /// Scroll container offset to the `ScrollView`.
    pub pos_y: f32,

    /// Maximum value for the scroll. It is updated automatically based on the size of the children nodes.
    pub max_scroll: f32,
}

impl ScrollableContent {
    /// Scrolls to the top of the scroll view.
    pub fn scroll_to_top(&mut self) {
        self.pos_y = 0.0;
    }
    /// Scrolls to the bottom of the scroll view.
    pub fn scroll_to_bottom(&mut self) {
        self.pos_y = -self.max_scroll;
    }
}

/// A temporary component setting the target value of the entity's `ScrollableContent` `y_pos`.
#[derive(Component, Debug, Reflect, Default)]
pub struct ScrollTarget {
    /// Target value for `ScrollableContent` `y_pos`
    pub target_y: f32,
    /// Was this target set by a scroll bar?
    pub set_by_scrollbar: bool,
}

impl ScrollTarget {
    /// Initializes a new ScrollTarget.
    ///
    /// # Parameters
    /// - `value`: The target value to scroll vertically. Positive values scroll down,
    ///   and negative values scroll up.
    pub fn from_value(value: f32, max_scroll: f32, scrollbar: bool) -> Self {
        let mut ret = Self::default();
        ret.scroll_by(value, max_scroll);
        ret.set_by_scrollbar = scrollbar;
        ret
    }

    /// Sets the scroll target by a specified amount.
    ///
    /// # Parameters
    /// - `value`: The target value to scroll vertically. Positive values scroll down,
    ///   and negative values scroll up.
    ///
    /// Ensures the new position is clamped using `max_scroll`.
    pub fn scroll_by(&mut self, value: f32, max_scroll: f32) {
        self.target_y += value;
        self.target_y = self.target_y.clamp(-max_scroll, 0.);
    }
}

/// Creates a default scroll view node.
///
/// This function defines the visual and layout properties of a scrollable container.
pub fn scroll_view_node() -> Node {
    Node {
        overflow: Overflow::hidden_y(),
        align_items: AlignItems::Start,
        flex_direction: FlexDirection::Row,
        ..default()
    }
}

/// Creates a default scroll content node.
pub fn scroll_content_node() -> Node {
    Node {
        flex_direction: bevy::ui::FlexDirection::Column,
        width: Val::Percent(100.0),
        ..default()
    }
}

/// Applies the default scroll view style to newly added `ScrollView` components.
///
/// This function updates the style of all new `ScrollView` nodes with the default
/// properties defined in `scroll_view_node`.
pub fn create_scroll_view(mut q: Query<&mut Node, Added<ScrollView>>) {
    let Node {
        overflow,
        align_items,
        flex_direction,
        ..
    } = scroll_view_node();
    for mut style in q.iter_mut() {
        style.overflow = overflow;
        style.align_items = align_items;
        style.flex_direction = flex_direction;
    }
}

fn input_mouse_pressed_move(
    mut motion_evr: MessageReader<MouseMotion>,
    mut q: Query<(&Children, &Interaction), With<ScrollView>>,
    mut commands: Commands,
    content_q: Query<&ScrollableContent>,
    mut target_q: Query<&mut ScrollTarget>,
) {
    for evt in motion_evr.read() {
        for (children, &interaction) in q.iter_mut() {
            if interaction != Interaction::Pressed {
                continue;
            }
            let y = evt.delta.y;
            set_scroll_targets(children, y, &mut commands, &content_q, &mut target_q);
        }
    }
}

fn update_size(
    mut q: Query<(&Children, &ComputedNode), With<ScrollView>>,
    mut content_q: Query<(&mut ScrollableContent, &ComputedNode), Changed<ComputedNode>>,
) {
    for (children, scroll_view_node) in q.iter_mut() {
        let container_height = (scroll_view_node.size().y
            - scroll_view_node.padding.top
            - scroll_view_node.padding.bottom)
            * scroll_view_node.inverse_scale_factor();
        for child in children.iter() {
            let Ok((mut scroll, node)) = content_q.get_mut(child) else {
                continue;
            };

            scroll.max_scroll =
                (node.size().y * node.inverse_scale_factor() - container_height).max(0.0);
            #[cfg(feature = "extra_logs")]
            info!(
                "CONTAINER {}, max_scroll: {}",
                container_height, scroll.max_scroll
            );
        }
    }
}

fn input_touch_pressed_move(
    touches: Res<Touches>,
    mut q: Query<(&Children, &Interaction), With<ScrollView>>,
    mut commands: Commands,
    content_q: Query<&ScrollableContent>,
    mut target_q: Query<&mut ScrollTarget>,
) {
    for t in touches.iter() {
        let Some(touch) = touches.get_pressed(t.id()) else {
            continue;
        };

        for (children, &interaction) in q.iter_mut() {
            if interaction != Interaction::Pressed {
                continue;
            }
            let y = touch.delta().y;
            set_scroll_targets(children, y, &mut commands, &content_q, &mut target_q);
        }
    }
}

fn scroll_events(
    mut scroll_evr: MessageReader<MouseWheel>,
    mut q: Query<(&Children, &Interaction, &ScrollView), With<ScrollView>>,
    time: Res<Time>,
    mut commands: Commands,
    content_q: Query<&ScrollableContent>,
    mut target_q: Query<&mut ScrollTarget>,
) {
    use bevy::input::mouse::MouseScrollUnit;
    for ev in scroll_evr.read() {
        for (children, &interaction, scroll_view) in q.iter_mut() {
            if interaction != Interaction::Hovered {
                continue;
            }
            let y = match ev.unit {
                MouseScrollUnit::Line => {
                    ev.y * time.delta().as_secs_f32() * scroll_view.scroll_speed
                }
                MouseScrollUnit::Pixel => ev.y,
            };
            #[cfg(feature = "extra_logs")]
            info!("Scrolling by {:#?}: {} movement", ev.unit, y);

            set_scroll_targets(children, y, &mut commands, &content_q, &mut target_q);
        }
    }
}

fn set_scroll_targets(
    children: &[Entity],
    y: f32,
    commands: &mut Commands,
    content_q: &Query<&ScrollableContent>,
    target_q: &mut Query<&mut ScrollTarget>,
) {
    for child in children.iter() {
        let Ok(scroll) = content_q.get(*child) else {
            continue;
        };
        if let Ok(mut target) = target_q.get_mut(*child) {
            target.set_by_scrollbar = false;
            target.scroll_by(y, scroll.max_scroll);
        } else {
            commands.entity(*child).try_insert(ScrollTarget::from_value(
                scroll.pos_y + y,
                scroll.max_scroll,
                false,
            ));
        }
    }
}

fn scroll_update(
    mut commands: Commands,
    mut q: Query<(Entity, &mut ScrollableContent, &ScrollTarget, &mut Node)>,
    time: Res<Time>,
) {
    for (e, mut scroll, target, mut style) in q.iter_mut() {
        scroll.pos_y +=
            (target.target_y - scroll.pos_y) * time.delta_secs() * (f32::consts::PI * 2.0);
        style.top = Val::Px(scroll.pos_y);
        if (target.target_y - scroll.pos_y).abs() < 0.01 {
            commands.entity(e).try_remove::<ScrollTarget>();
        }
    }
}
