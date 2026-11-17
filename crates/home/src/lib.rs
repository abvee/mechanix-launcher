use gestures;
use mecha_wayland::prelude::*;
use screen::Screen;

/// Longer side of the Comet wheel, logical pixels.
const LOGO_SIZE: u32 = 120;

pub struct HomeScreen;

impl Screen for HomeScreen {
    fn build(&self, app: &mut App, parent: NodeId) -> NodeId {
        app.spawn(parent, Home { drag: None }).id()
    }
}

struct Drag {
    contact: ContactId,
    point: Point,
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
        b
    }
}

#[cfg(test)]
mod tests;
