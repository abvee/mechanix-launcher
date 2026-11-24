//! Logical grid placement and the positioning wrapper for a homescreen item.

use mecha_wayland::mechanix_widgets::prelude::{TextVariant, text};
use mecha_wayland::prelude::*;

use crate::page::{HomePageModeChanged, PageMode};

const EDIT_BADGE_SIZE: f32 = 24.0;
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
/// spawned beneath it and manages its own internal layout.
pub(super) struct HomeItem {
    pub(super) placement: Placement,
    /// Hidden until the page's mode-change handler shows it.
    pub(super) edit_badge: Handle<Div>,
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

        spawn_placeholder(s, me, item.number);
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
        HomeItem {
            placement: item.placement,
            edit_badge,
        }
    }
}

/// Temporary content, inset from the grid footprint to leave space between cards.
/// Keeping it beneath the wrapper lets placement and dragging move it as a unit.
fn spawn_placeholder<W: Widget>(s: &mut Spawner<'_, W>, item: Handle<HomeItem>, number: usize) {
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
