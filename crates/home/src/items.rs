//! Logical grid placement and the positioning wrapper for a homescreen item.

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
/// The page assigns this node's position and size. Future icon/widget content
/// will be spawned beneath it and manage its own internal layout.
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

        // Red temporarily exposes the allocated rectangle. Once content is
        // spawned beneath the wrapper, use Paint::None for the wrapper itself.
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(Quad::new(Color::rgb(1.0, 0.0, 0.0)));
        item
    }
}
