//! A full-size home page with percentage-positioned items and cell occupancy.
//!
//! Item placements are authoritative; occupancy maps each covered cell to an
//! index in the page's item vector. A centred SVG temporarily identifies the page.

use crate::items::*;
use mecha_wayland::prelude::*;

/// Longer side of the page icon, in logical pixels.
const LOGO_SIZE: u32 = 120;
/// Grid dimensions used for both percentage placement and occupancy indexing.
const GRID_ROWS: usize = 4;
const GRID_COLUMNS: usize = 4;

/// Supplies the temporary identifying icon for a page.
pub(crate) struct HomePageBuilder {
    /// Embedded SVG bytes used to distinguish pages.
    /// Assets loaded with `include_bytes!()` have a `'static` lifetime.
    // TODO: Replace the identifying icon with wallpaper.
    pub(super) svg_icon: &'static [u8],
}
impl Build for HomePageBuilder {
    type Widget = HomePage;
}

#[derive(Copy, Clone)]
enum PageMode {
    Normal,
    Edit,
}

/// Page content: a background icon and items placed on a fixed grid.
/// Builds hidden; Home owns which page is displayed.
pub(crate) struct HomePage {
    /// Item nodes in index order. Occupancy entries refer to this vector.
    items: Vec<Handle<HomeItem>>,
    /// Row-major cell ownership: `None` is empty; `Some(index)` refers to
    /// `items[index]`. A multi-cell item repeats its index across its footprint.
    /// Rebuild after adding, removing, reordering, or changing item placements.
    /// Every stored item index must fit in `u8`.
    occupancy: [[Option<u8>; GRID_COLUMNS]; GRID_ROWS],

    mode: PageMode, // change mode on long press. Each mode has a different way of handling the same state
}
impl Widget for HomePage {
    type Builder = HomePageBuilder;

    fn build(builder: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default()
            // Keep the page in normal flow; its items are positioned absolutely.
            .relative()
            .center()
            .width(percent(100.0))
            .height(percent(100.0))
            .display(Display::Hidden);

        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(Quad::new(Color::BLACK));

        // Spawn the background icon before items so their content draws above it.
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
        // TODO: Validate the entire placement list before spawning items when
        // loading configuration. Bounds and collisions are not checked below.

        let mut items: Vec<Handle<HomeItem>> = Vec::new();
        let mut occupancy = [[None; GRID_COLUMNS]; GRID_ROWS];

        for (i, placement) in placements.into_iter().enumerate() {
            let style = item_style(&placement);

            // Record every occupied cell using the item's index in spawn order.
            // TODO: Share this logic with occupancy rebuilding after item edits.
            for row in placement.row..placement.row + placement.height {
                for column in placement.column..placement.column + placement.width {
                    occupancy[row][column] =
                        Some(u8::try_from(i).expect("item index MUST fit in u8"))
                }
            }

            let item = s.spawn(me, HomeItem(placement));
            // The page assigns the wrapper's percentage-based position and size.
            *s.component_mut::<LayoutStyle>(item).unwrap() = style;

            items.push(item);
        }

        HomePage {
            items,
            occupancy,
            mode: PageMode::Normal,
        }
    }
}

/// Convert a grid footprint into a percentage-based rectangle within the page.
/// The caller supplies a valid placement. This only calculates layout; it does
/// not validate bounds, detect collisions, or update occupancy.
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

#[cfg(test)]
#[path = "../tests/unit/page.rs"]
mod tests;
