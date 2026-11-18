//! Home's screen and widget-level horizontal gesture handling.
//!
//! The screen currently shows the Comet logo. It tracks one contact at a
//! time, claims horizontal drags, and logs their direction on release.
//! Page switching is not implemented yet. Vertical gestures remain unclaimed
//! here so other handlers, including shell navigation, can interpret them.

use mecha_wayland::prelude::*;
use screen::Screen;

/// Longer side of the Comet wheel, logical pixels.
const LOGO_SIZE: u32 = 120;

/// Movement in logical pixels before choosing an axis, not a page-switch
/// distance. Either axis must exceed this threshold before it can lock.
const AXIS_LOCK_THRESHOLD: f32 = 10.0;

/// Builds Home under the supplied parent; the host owns the window and
/// installs layout, paint, interactivity, the theme, and an atlas.
pub struct HomeScreen;

impl Screen for HomeScreen {
    fn build(&self, app: &mut App, parent: NodeId) -> NodeId {
        app.spawn(parent, Home { drag: None }).id()
    }
}

/// One press, tracked until its matching release or cancellation.
struct Drag {
    contact: ContactId,
    /// Window-local position of the original press; deltas are measured from it.
    start_point: Point,
    axis: gestures::AxisLock,
    /// Home successfully took ownership. Prevents repeated claims and logs.
    claimed: bool,
}

/// The persistent screen widget; its pending gesture is independent of layout.
struct Home {
    drag: Option<Drag>,
}

impl Build for Home {
    type Widget = Home;
}

impl Widget for Home {
    type Builder = Home;

    fn build(b: Self, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .center()
            .width(percent(100.0))
            .height(percent(100.0));
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(Quad::new(Color::BLACK));

        let logo = s
            .resource_mut::<Atlas>()
            .insert_svg(assets::icons::COMET_LOGO, LOGO_SIZE)
            .expect("embedded Comet logo is a valid SVG");
        let color = s.color(ColorRole::OnSurface);
        s.spawn(me, icon(logo).color(color));

        s.on::<Press>(me, on_press);
        s.on::<Release>(me, on_release);
        s.on::<PressCancelled>(me, on_press_cancelled);
        s.on::<PressMoved>(me, on_press_moved);
        b
    }
}

/// Observe a press without claiming it: children can still receive a tap.
fn on_press(ctx: &mut Context<'_, Home>, e: &Press) {
    let home = ctx.me();
    if home.drag.is_some() {
        // Another contact already drives this gesture.
        return;
    }

    home.drag = Some(Drag {
        contact: e.contact,
        start_point: e.position,
        axis: gestures::AxisLock::new(AXIS_LOCK_THRESHOLD),
        claimed: false,
    });
}

/// Report the final direction only for a claimed horizontal drag, then
/// forget it. A release from an ignored second finger must not end this drag.
fn on_release(ctx: &mut Context<'_, Home>, e: &Release) {
    let home = ctx.me();
    let Some(drag) = home.drag.as_mut() else {
        return;
    };

    if drag.contact != e.contact {
        return;
    }

    if drag.claimed {
        let dx = e.position.x - drag.start_point.x;

        if dx < 0.0 {
            tracing::info!("Left swipe");
        } else if dx > 0.0 {
            tracing::info!("Right swipe");
        }
    }

    home.drag = None;
}

/// Lock direction using total displacement, then claim horizontal movement
/// once. Claiming cancels other captured nodes' presses, preventing child taps.
fn on_press_moved(ctx: &mut Context<'_, Home>, e: &PressMoved) {
    let home = ctx.me();
    let Some(drag) = home.drag.as_mut() else {
        return;
    };

    if drag.contact != e.contact || drag.claimed {
        return;
    }

    let dx = e.position.x - drag.start_point.x;
    let dy = e.position.y - drag.start_point.y;

    match drag.axis.update(dx, dy) {
        None => {} // Not enough movement to choose an axis yet.
        Some(gestures::Axis::Horizontal) => {
            if ctx.claim_contact(e.contact) {
                // Claiming changes capture immediately; cancellation handlers
                // for the other nodes are queued, so our drag still exists.
                ctx.me().drag.as_mut().unwrap().claimed = true;
                tracing::info!("Claimed horizontal swipe with contact: {:?}", e.contact);
            }
        }
        Some(gestures::Axis::Vertical) => {} // The axis stays vertical for this press.
    }
}

/// A cancelled press receives no release. Clear it here so the next press
/// can start, including when a child claims the contact for its own gesture.
fn on_press_cancelled(ctx: &mut Context<'_, Home>, e: &PressCancelled) {
    let home = ctx.me();
    if home
        .drag
        .as_ref()
        .is_some_and(|drag| drag.contact == e.contact)
    {
        home.drag = None;
    }
}

#[cfg(test)]
mod tests;
