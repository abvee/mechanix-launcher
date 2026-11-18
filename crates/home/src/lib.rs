use gestures;
use mecha_wayland::prelude::*;
use screen::Screen;

/// Longer side of the Comet wheel, logical pixels.
const LOGO_SIZE: u32 = 120;

const HORIZONTAL_SWIPE_THRESHOLD: f32 = 10.0; // in pixels

pub struct HomeScreen;

impl Screen for HomeScreen {
    fn build(&self, app: &mut App, parent: NodeId) -> NodeId {
        app.spawn(parent, Home { drag: None }).id()
    }
}

struct Drag {
    contact: ContactId,
    start_point: Point,
    axis: gestures::AxisLock,
}

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

fn on_press(ctx: &mut Context<'_, Home>, e: &Press) {
    let home = ctx.me();
    if home.drag.is_some() {
        // we already have a drag
        return;
    }

    home.drag = Some(Drag {
        contact: e.contact,
        start_point: e.position,
        axis: gestures::AxisLock::new(HORIZONTAL_SWIPE_THRESHOLD),
    });
}

fn on_release(ctx: &mut Context<'_, Home>, e: &Release) {
    let home = ctx.me();

    if home
        .drag
        .as_ref()
        .is_some_and(|drag| drag.contact == e.contact)
    {
        home.drag = None;
    }
}

fn on_press_moved(ctx: &mut Context<'_, Home>, e: &PressMoved) {
    let home = ctx.me();
    let Some(drag) = home.drag.as_mut() else {
        return;
    };

    if drag.contact != e.contact {
        return;
    }

    let dx = e.position.x - drag.start_point.x;
    let dy = e.position.y - drag.start_point.y;

    match drag.axis.update(dx, dy) {
        None => {} // Not enough movement to choose an axis yet
        Some(gestures::Axis::Horizontal) => {
            ctx.claim_contact(e.contact);
            tracing::info!("Claimed horizontal swipe with contact: {:?}", e.contact);
        }
        _ => {} // leave all vertical gestures alone
    }
}

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
