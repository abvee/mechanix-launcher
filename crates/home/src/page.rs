//! A full-size home page with a temporary centred icon to distinguish it.

use mecha_wayland::prelude::*;

/// Longer side of the page icon, in logical pixels.
const LOGO_SIZE: u32 = 120;

/// Supplies the temporary identifying icon for a page.
pub(crate) struct HomePageBuilder {
    /// Embedded SVG bytes used to distinguish pages.
    /// Assets loaded with `include_bytes!()` have a `'static` lifetime.
    pub(super) svg_icon: &'static [u8],
}
impl Build for HomePageBuilder {
    type Widget = HomePage;
}

/// Page content: a black background and a centred identifying icon.
pub(crate) struct HomePage;
impl Widget for HomePage {
    type Builder = HomePageBuilder;

    fn build(builder: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .center()
            .width(percent(100.0))
            .height(percent(100.0))
            .display(Display::Hidden); // start all pages as hidden, this will be controlled by Home
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(Quad::new(Color::BLACK));

        let logo = s
            .resource_mut::<Atlas>()
            .insert_svg(builder.svg_icon, LOGO_SIZE)
            .expect("embedded logo is a valid SVG");

        let color = s.color(ColorRole::OnSurface);
        s.spawn(me, icon(logo).color(color));

        HomePage
    }
}
