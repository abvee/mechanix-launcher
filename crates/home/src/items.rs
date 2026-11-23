//! Logical grid placement and the positioning wrapper for a homescreen item.

use mecha_wayland::mechanix_widgets::prelude::{TextVariant, text};
use mecha_wayland::prelude::*;

/// An item's footprint in grid cells, independent of the page's pixel size.
/// Coordinates are zero-based; spans must be nonzero and fit within the grid.
/// The page is responsible for validating placements before using them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Row containing the item's top edge.
    pub row: usize,
    /// Column containing the item's left edge.
    pub column: usize,

    /// Number of rows occupied.
    pub height: usize,
    /// Number of columns occupied.
    pub width: usize,
}

/// A wrapper holding an item's logical placement.
///
/// The page assigns this node's position and size. Icon/widget content is
/// spawned beneath it and manages its own internal layout.
// TODO: make this not a tuple struct because we will also be passing the
// HomeWidget builder here
pub(super) struct HomeItem(pub(super) Placement);

impl Build for HomeItem {
    type Widget = HomeItem;
}

impl Widget for HomeItem {
    type Builder = HomeItem;
    fn build(item: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default().absolute();

        *s.component_mut::<Paint>(me).unwrap() = Paint::None;
        item
    }
}

/// Temporary content, inset from the grid footprint to leave space between cards.
/// Keeping it beneath the wrapper lets placement and dragging move it as a unit.
pub(super) fn spawn_placeholder<W: Widget>(
    s: &mut Spawner<'_, W>,
    item: Handle<HomeItem>,
    number: usize,
) {
    let card = s.spawn(
        item,
        div().style(
            LayoutStyle::default()
                .absolute()
                .inset(Insets::all(px(8.0)))
                .center(),
        ),
    );
    let background = s.color(ColorRole::SurfaceContainer);
    let border = s.color(ColorRole::OutlineVariant);
    *s.component_mut::<Paint>(card).unwrap() =
        Paint::Quad(Quad::new(background).radius(16.0).border(1.0, border));
    s.spawn(
        card,
        text(format!("Widget {number}"))
            .variant(TextVariant::TitleMedium)
            .weight(FontWeight::Regular)
            .color(ColorRole::Primary)
            .max_lines(1),
    );
}
