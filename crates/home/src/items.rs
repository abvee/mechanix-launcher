//! Logical grid placement and the positioning wrapper for a homescreen item.

use mecha_wayland::mechanix_widgets::prelude::{TextVariant, text};
use mecha_wayland::prelude::*;

use crate::page::{HomePageModeChanged, PageMode};

const EDIT_BADGE_SIZE: f32 = 24.0;
/// Minimum space around the square content, reserved for editing controls.
const CONTENT_INSETS: f32 = 28.0;

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
/// spawned beneath it and manages its own internal layout. HomeItem assigns
/// that child's outer geometry and owns the editing controls.
pub(super) struct HomeItem {
    pub(super) placement: Placement,
    /// Hidden until the page's mode-change handler shows it.
    pub(super) edit_badge: Handle<Div>,

    /// The actual widget child.
    content: Handle<Div>,
}

pub(super) struct HomeItemBuilder {
    pub(super) placement: Placement,
    pub(super) number: usize,
}

impl Build for HomeItemBuilder {
    type Widget = HomeItem;
}

impl Widget for HomeItem {
    type Builder = HomeItemBuilder;
    fn build(item: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default().absolute();
        *s.component_mut::<Paint>(me).unwrap() = Paint::None;

        let content = spawn_placeholder(s, me.id(), item.number);
        {
            let mut style = s.component_mut::<LayoutStyle>(content).unwrap();
            style.position = Position::Absolute;
            style.inset = Insets::all(px(CONTENT_INSETS));
        }

        // Spawn after the card so the badge draws above its top-right corner.
        // fixed
        let edit_badge = s.spawn(
            me,
            div().style(
                LayoutStyle::default()
                    .absolute()
                    .inset(Insets::new(px(0.0), px(0.0), auto(), auto()))
                    .size(px(EDIT_BADGE_SIZE), px(EDIT_BADGE_SIZE))
                    .center()
                    .display(Display::Hidden),
            ),
        );
        let background = s.color(ColorRole::Primary);
        *s.component_mut::<Paint>(edit_badge).unwrap() =
            Paint::Quad(Quad::new(background).radius(EDIT_BADGE_SIZE / 2.0));
        s.spawn(
            edit_badge,
            text("×") // fixed
                .variant(TextVariant::LabelLarge)
                .weight(FontWeight::Regular)
                .size(18.0)
                .color(ColorRole::OnPrimary)
                .max_lines(1),
        );

        s.on::<HomePageModeChanged>(me, on_mode_changed);

        // Only here to determine insets.
        s.on::<OnChanged<ComputedLayout>>(me, on_layout_changed);
        HomeItem {
            placement: item.placement,
            edit_badge,
            content,
        }
    }
}

/// Temporary widget appearance and internal layout; HomeItem assigns its insets.
/// Keeping it beneath the wrapper lets placement and dragging move it as a unit.
fn spawn_placeholder<W: Widget>(
    s: &mut Spawner<'_, W>,
    parent: NodeId,
    number: usize,
) -> Handle<Div> {
    let card = s.spawn(parent, div().style(LayoutStyle::default().center()));
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
    card
}

fn on_mode_changed(ctx: &mut Context<'_, HomeItem>, e: &HomePageModeChanged) {
    let badge = ctx.me().edit_badge;
    ctx.at(badge)
        .unwrap()
        .component_mut::<LayoutStyle>()
        .unwrap()
        .display = match e.0 {
        PageMode::Normal => Display::Hidden,
        PageMode::Edit => Display::Flex,
    };
}

// TODO: Replace this handler with declarative aspect-ratio sizing when
// LayoutStyle supports fitting a square within both available dimensions.
// Fixed percentage insets alone cannot make a rectangular footprint square.
// Keep space around the content for the x button and resize handle bars.
fn on_layout_changed(ctx: &mut Context<'_, HomeItem>, _: &OnChanged<ComputedLayout>) {
    // This also runs when the wrapper moves or a page is hidden/shown, even on
    // a fixed-size launcher. Desktop window resizing changes the size as well.
    // TODO: Cache the last size per HomeItem to skip position-only changes.
    // A global/static run-once flag would incorrectly share state across items
    // and prevent later resizes from updating their content.
    let (width, height) = {
        let rect = ctx.component::<ComputedLayout>().unwrap().rect;
        (rect.width(), rect.height())
    };

    if width <= 0.0 || height <= 0.0 {
        return;
    }

    // the side of the inner square
    let side = (width.min(height) - 2.0 * CONTENT_INSETS).max(0.0);

    // spacing on the top/bottom and left/right
    let horizontal = (width - side) / 2.0;
    let vertical = (height - side) / 2.0;

    let content = ctx.me().content;

    ctx.at(content)
        .unwrap()
        .component_mut::<LayoutStyle>()
        .unwrap()
        .inset = Insets::new(px(vertical), px(horizontal), px(vertical), px(horizontal));
}

#[cfg(test)]
#[path = "../tests/unit/items.rs"]
mod tests;
