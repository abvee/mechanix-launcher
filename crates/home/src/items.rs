//! Logical grid placement and the positioning wrapper for a homescreen item.

use mecha_wayland::mechanix_widgets::prelude::{
    Button, ButtonSize, ButtonVariant, TextVariant, button, text,
};
use mecha_wayland::prelude::*;

use crate::page::{HomePageModeChanged, PageMode};

/// Edge length of the × button, in logical pixels.
const X_BUTTON_SIZE: f32 = 24.0;
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
    /// Committed grid footprint. The page validates it and assigns the
    /// wrapper's rectangle from it.
    pub(super) placement: Placement,
    /// Hidden until the page's mode-change handler shows it.
    pub(super) x_button: Handle<Button>,

    /// The actual widget child. `pub(super)` lets the page's tests locate
    /// the draggable content rect; production code uses
    /// [`HomeItem::can_drag_at`] instead of reading it directly.
    pub(super) content: Handle<Div>,
}

/// Builds one item: its committed grid placement and the placeholder's label
/// number. Removal identifies items by handle, not by this number.
pub(super) struct HomeItemBuilder {
    /// Committed grid footprint within the page.
    pub(super) placement: Placement,
    /// Temporary: only names the placeholder card.
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

        // Spawn after the card so the x button draws above its top-right
        // corner.
        let x_button = s.spawn(
            me,
            button()
                .variant(ButtonVariant::Filled)
                .size(ButtonSize::Small)
                .label("×")
                .radius(Corners::all(X_BUTTON_SIZE / 2.0))
                // inner native button fills out outer node
                .style(LayoutStyle::default().fill().center()),
        );

        // position the X button wherever we want, we also control it's
        // visibility
        *s.component_mut::<LayoutStyle>(x_button).unwrap() = LayoutStyle::default()
            .absolute()
            // TODO: maybe force this to match widgets insets ? Will need to
            // come inside OnChanged<ComputedLayout>
            .inset(Insets::new(px(0.0), px(0.0), auto(), auto()))
            .size(px(X_BUTTON_SIZE), px(X_BUTTON_SIZE))
            .display(Display::Hidden);
        s.on::<Press>(x_button, |ctx, e| {
            if ctx.claim_contact(e.contact) {
                tracing::info!("(x) badge pressed: {:?}", e.contact);
            }
        })
        /*
        WARNING / TO FIX: If we ever change the way that Clicked works (only to
        mouse and not released along side Release), this will need to be fixed.
        Release, PressCancelled and PressMoved would have to be added OR
        replace all interactivity events with a single Tap
        */
        .on::<Clicked>(x_button, move |ctx, _| {
            // NOTE: .parent() here implicitly assumes HomeItem's direct parent
            // will handle removal. If this changes, use
            // ctx.tree().ancestors().find()
            ctx.emit(RemoveItemRequest(me), ctx.tree().parent(me).unwrap());
            tracing::info!("X button clicked, removal requested for item id: {:?}", me);
        });

        s.on::<HomePageModeChanged>(me, on_mode_changed);

        // Only here to determine insets.
        s.on::<OnChanged<ComputedLayout>>(me, on_layout_changed);
        HomeItem {
            placement: item.placement,
            x_button,
            content,
        }
    }
}

/// An item's × button asked its owning page to remove it. The page owns the
/// decision and all bookkeeping: the item list, occupancy, and the tree.
pub(super) struct RemoveItemRequest(pub(super) Handle<HomeItem>);
impl Event for RemoveItemRequest {}

impl HomeItem {
    /// Check whether a window-local point lies inside the content, excluding
    /// the surrounding space reserved for editing controls. Right and bottom
    /// edges are exclusive, so zero-sized content cannot match.
    pub fn can_drag_at(ctx: &mut Context<'_, Self>, point: Point) -> bool {
        let content = ctx.me().content;
        let rect = ctx
            .at(content)
            .unwrap()
            .component::<ComputedLayout>()
            .unwrap()
            .rect;

        rect.x() <= point.x
            && point.x < rect.x() + rect.width()
            && rect.y() <= point.y
            && point.y < rect.y() + rect.height()
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

/// Show the × button in Edit mode, hide it in Normal.
fn on_mode_changed(ctx: &mut Context<'_, HomeItem>, e: &HomePageModeChanged) {
    let badge = ctx.me().x_button;
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
