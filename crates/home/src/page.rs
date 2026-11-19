//! A full-size home page with a temporary centred icon to distinguish it.

use crate::items::*;
use mecha_wayland::prelude::*;

/// Longer side of the page icon, in logical pixels.
const LOGO_SIZE: u32 = 120;
const GRID_ROWS: u32 = 4;
const GRID_COLUMNS: u32 = 4;

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
/// Builds hidden; Home owns which page is displayed.
pub(crate) struct HomePage;
impl Widget for HomePage {
    type Builder = HomePageBuilder;

    fn build(builder: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            .relative()
            /*
            This ^  is purposefully explicit. We want each page to derive
            it's layout from Home, so that we can use percentages when doing
            the widget grid.
            */
            .center()
            .width(percent(100.0))
            .height(percent(100.0))
            .display(Display::Hidden);

        // background
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(Quad::new(Color::BLACK));

        // spawn logo
        let logo = s
            .resource_mut::<Atlas>()
            .insert_svg(builder.svg_icon, LOGO_SIZE)
            .expect("embedded logo is a valid SVG");

        let color = s.color(ColorRole::OnSurface);
        s.spawn(me, icon(logo).color(color));

        // Temporary, non-overlapping placements with nonzero spans inside the grid.
        let placements = [
            Placement {
                row: 0,
                column: 0,
                width: 2,
                height: 2,
            },
            Placement {
                row: 3,
                column: 3,
                width: 1,
                height: 1,
            },
        ];

        // TODO: validate positions here. Ideally we would get these from a
        // config file at this point

        for placement in placements {
            let style = item_style(&placement);
            let item = s.spawn(me, HomeItem(placement));
            *s.component_mut::<LayoutStyle>(item).unwrap() = style; // sets the position
        }

        HomePage
    }
}

/// convert the placement grid into a rectangle with percentages.
/// Caller has to supply a valid placement struct
/// Only call afer you verify that no collisions are possible
fn item_style(placement: &Placement) -> LayoutStyle {
    let column_percent = 100.0 / GRID_COLUMNS as f32;
    let row_percent = 100.0 / GRID_ROWS as f32;

    LayoutStyle::default()
        .absolute()
        .inset(Insets::new(
            percent(placement.row as f32 * row_percent),
            auto(),
            auto(),
            percent(placement.column as f32 * column_percent),
        ))
        .size(
            percent(placement.width as f32 * column_percent),
            percent(placement.height as f32 * row_percent),
        )
}
