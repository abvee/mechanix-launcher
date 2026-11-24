//! A full-size home page with percentage-positioned items and cell occupancy.
//!
//! Item placements are authoritative; occupancy maps each covered cell to an
//! index in the page's item vector. A centred SVG temporarily identifies the page.
//! A drag changes wrapper styles immediately, but only commits placements and
//! occupancy on release. Cancellation restores the original arrangement.

use crate::grid::{self, GRID_COLUMNS, GRID_ROWS, Occupancy};
use crate::items::*;
use mecha_wayland::prelude::*;
use std::time::Duration;

/// Longer side of the page icon, in logical pixels.
const LOGO_SIZE: u32 = 120;
const LONG_PRESS_DELAY: Duration = Duration::from_millis(250);
/// Maximum pending long-press movement, in logical pixels.
const LONG_PRESS_SLOP: f32 = 5.0;

/// Supplies the temporary identifying icon for a page.
pub(crate) struct HomePageBuilder {
    /// Embedded SVG bytes used to distinguish pages.
    /// Assets loaded with `include_bytes!()` have a `'static` lifetime.
    // TODO: Replace the identifying icon with wallpaper.
    pub(super) svg_icon: &'static [u8],
    /// Populate this page with the temporary demonstration widgets.
    pub(super) debug_items: bool,
}
impl Build for HomePageBuilder {
    type Widget = HomePage;
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(super) enum PageMode {
    Normal,
    Edit,
}

/// Emit to HomeItems whenever we change modes.
pub(super) struct HomePageModeChanged(pub(super) PageMode);
impl Event for HomePageModeChanged {}

struct LongPress {
    contact: ContactId,
    /// Window-local position of the original press; deltas are measured from it.
    start_point: Point,
    /// Latest position within the slop, so starting a drag follows the finger.
    position: Point,
    /// The page successfully took ownership. Prevents repeated mode toggles.
    claimed: bool,
}

struct ItemDrag {
    contact: ContactId,
    item_index: usize,
    /// Where the finger grabbed the wrapper, relative to its top-left corner.
    grab_offset: Point,
    original: Vec<Placement>,
    candidate: Placement,
    /// None means the current candidate cannot be arranged by our grid policy.
    preview: Option<Vec<Placement>>,
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
    occupancy: Occupancy,
    mode: PageMode,
    /// Pending or recognized mode-toggle press; separate from item dragging.
    long_press: Option<LongPress>,
    long_press_timer: TimerId,
    drag: Option<ItemDrag>,
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

        let long_press_timer = s.add_timeout(LONG_PRESS_DELAY, on_long_press_timeout);
        s.on::<Press>(me, on_press)
            .on::<Release>(me, on_release)
            .on::<PressMoved>(me, on_press_moved)
            .on::<PressCancelled>(me, on_press_cancelled);

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
            Placement {
                row: 7,
                column: 0,
                width: 2,
                height: 1,
            },
        ];
        let placements = if builder.debug_items {
            placements.as_slice()
        } else {
            &[]
        };
        let occupancy = grid::occupancy(placements).expect("valid initial item placements");
        let mut items: Vec<Handle<HomeItem>> = Vec::new();

        for (index, &placement) in placements.iter().enumerate() {
            let style = item_style(&placement);

            let item = s.spawn(
                me,
                HomeItemBuilder {
                    placement,
                    number: index + 1,
                },
            );
            // The page assigns the wrapper's percentage-based position and size.
            *s.component_mut::<LayoutStyle>(item).unwrap() = style;

            items.push(item);
        }

        HomePage {
            items,
            occupancy,
            mode: PageMode::Normal,
            long_press: None,
            long_press_timer,
            drag: None,
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

fn on_press(ctx: &mut Context<'_, HomePage>, e: &Press) {
    let page = ctx.me();

    if page.long_press.is_some() || page.drag.is_some() {
        return;
    }

    // In Edit mode, grabbing an item does not require another long press.
    if page.mode == PageMode::Edit {
        if let Some(index) = item_at(ctx, e.position) {
            if ctx.claim_contact(e.contact) {
                start_drag(ctx, e.contact, index, e.position, e.position);
            }
            return;
        }
    }

    let page = ctx.me();
    page.long_press = Some(LongPress {
        contact: e.contact,
        start_point: e.position,
        position: e.position,
        claimed: false,
    });

    // start long press timer
    let timer = page.long_press_timer;
    ctx.start_timer(timer);

    tracing::info!("Tracking possible long press: {:?}", e.contact);
}

// helper function for clearing long press and associated timer
fn clear_long_press(ctx: &mut Context<'_, HomePage>, contact: ContactId) {
    let timer = {
        let page = ctx.me();
        let removed = page.long_press.take_if(|press| press.contact == contact);

        if removed.is_none() {
            return;
        }

        page.long_press_timer
    };
    ctx.cancel_timer(timer);
}

fn on_release(ctx: &mut Context<'_, HomePage>, e: &Release) {
    // A release can carry a newer position than the last movement event.
    move_drag(ctx, e.contact, e.position);
    finish_drag(ctx, e.contact, true);
    clear_long_press(ctx, e.contact);
}

fn on_press_cancelled(ctx: &mut Context<'_, HomePage>, e: &PressCancelled) {
    finish_drag(ctx, e.contact, false);
    clear_long_press(ctx, e.contact);
}

fn on_press_moved(ctx: &mut Context<'_, HomePage>, e: &PressMoved) {
    if ctx.me().drag.is_some() {
        move_drag(ctx, e.contact, e.position);
        return;
    }
    let page = ctx.me();

    let Some(press) = page.long_press.as_mut() else {
        return;
    };

    if press.contact != e.contact || press.claimed {
        return;
    }

    let dx = e.position.x - press.start_point.x;
    let dy = e.position.y - press.start_point.y;
    press.position = e.position;

    // the old distance squared trick
    let distance_squared = dx * dx + dy * dy;
    if distance_squared > LONG_PRESS_SLOP * LONG_PRESS_SLOP {
        clear_long_press(ctx, e.contact);

        tracing::info!("Pending long press cancelled by movement: {:?}", e.contact);
    }
}

fn on_long_press_timeout(ctx: &mut Context<'_, HomePage>, _: &Timeout) {
    let timer = ctx.me().long_press_timer;

    // Ignore an older queued timeout if a new press restarted the countdown.
    if ctx.timer_running(timer) {
        return;
    }

    let (contact, start_point, position) = {
        let page = ctx.me();

        let Some(press) = page.long_press.as_ref() else {
            return;
        };

        // one transition per press
        if press.claimed {
            return;
        }

        (press.contact, press.start_point, press.position)
    };

    // Claim the contact
    // This has to obviously happen before we change mode
    if !ctx.claim_contact(contact) {
        return;
    }

    let (mode, targets) = {
        let page = ctx.me();
        page.long_press.as_mut().unwrap().claimed = true;
        page.mode = match page.mode {
            PageMode::Normal => PageMode::Edit,
            PageMode::Edit => PageMode::Normal,
        };
        let targets: Vec<NodeId> = page.items.iter().map(|item| item.id()).collect();
        tracing::info!("Long press done, switched mode to: {:?}", page.mode);
        (page.mode, targets)
    };

    ctx.emit(HomePageModeChanged(mode), targets);

    // you should be able to start dragging in the same long press
    if mode == PageMode::Edit {
        if let Some(index) = item_at(ctx, start_point) {
            start_drag(ctx, contact, index, start_point, position);
        }
    }
}

/// Find the wrapper under the original press, including future child content.
fn item_at(ctx: &mut Context<'_, HomePage>, point: Point) -> Option<usize> {
    let items = ctx.me().items.clone();
    items.iter().position(|&item| {
        let item_ctx = ctx.at(item).unwrap();
        let rect = item_ctx.component::<ComputedLayout>().unwrap().rect;
        rect.width() > 0.0
            && rect.height() > 0.0
            && point.x >= rect.x()
            && point.x < rect.x() + rect.width()
            && point.y >= rect.y()
            && point.y < rect.y() + rect.height()
    })
}

/// Called only after the page has claimed this contact.
fn start_drag(
    ctx: &mut Context<'_, HomePage>,
    contact: ContactId,
    item_index: usize,
    press: Point,
    position: Point,
) {
    let items = ctx.me().items.clone();
    let item = items[item_index];
    let rect = ctx
        .at(item)
        .unwrap()
        .component::<ComputedLayout>()
        .unwrap()
        .rect;
    let original: Vec<_> = items
        .iter()
        .map(|&item| ctx.widget::<HomeItem>(item).unwrap().placement)
        .collect();
    ctx.me().drag = Some(ItemDrag {
        contact,
        item_index,
        grab_offset: Point::new(press.x - rect.x(), press.y - rect.y()),
        candidate: original[item_index],
        preview: Some(original.clone()),
        original,
    });
    // Long-press state has done its job. Dragging now owns the contact.
    clear_long_press(ctx, contact);
    ctx.signal(RaiseItem {
        page: ctx.handle(),
        item,
    });
    move_drag(ctx, contact, position);
    tracing::info!("Started dragging item {item_index}");
}

fn move_drag(ctx: &mut Context<'_, HomePage>, contact: ContactId, position: Point) {
    let rect = ctx.component::<ComputedLayout>().unwrap().rect;
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let page = ctx.me();
    let Some(drag) = page.drag.as_mut().filter(|drag| drag.contact == contact) else {
        return;
    };
    let size = Size::new(rect.width(), rect.height());
    let footprint = drag.original[drag.item_index];
    let width = size.width * footprint.width as f32 / GRID_COLUMNS as f32;
    let height = size.height * footprint.height as f32 / GRID_ROWS as f32;
    let left = (position.x - rect.x() - drag.grab_offset.x).clamp(0.0, size.width - width);
    let top = (position.y - rect.y() - drag.grab_offset.y).clamp(0.0, size.height - height);
    let candidate = grid::candidate(footprint, left, top, size);
    let changed = candidate != drag.candidate;
    if changed {
        drag.candidate = candidate;
        drag.preview = grid::rearrange(&drag.original, drag.item_index, candidate);
    }
    let index = drag.item_index;
    let item = page.items[index];

    if changed {
        // Invalid candidate? Show the original neighbours, not a stale preview.
        let drag = ctx.me().drag.as_ref().unwrap();
        let placements = drag.preview.as_ref().unwrap_or(&drag.original).clone();
        apply_layout(ctx, &placements, Some(index));
    }

    // Only the dragged wrapper has pixel offsets; its size stays percentage-based.
    let mut item_ctx = ctx.at(item).unwrap();
    item_ctx.component_mut::<LayoutStyle>().unwrap().inset =
        Insets::new(px(top), auto(), auto(), px(left));
}

/// Apply a preview to existing wrappers. The dragged wrapper follows the finger.
fn apply_layout(ctx: &mut Context<'_, HomePage>, placements: &[Placement], skip: Option<usize>) {
    let items = ctx.me().items.clone();

    // TODO: Filter these based on what actually changed in the preview
    // placement and keep the others the same? We're currently writing styles
    // for every non-dragged item whenever the candidate changes (a 4x8 grid).
    for (index, (&item, placement)) in items.iter().zip(placements).enumerate() {
        if Some(index) != skip {
            *ctx.at(item)
                .unwrap()
                .component_mut::<LayoutStyle>()
                .unwrap() = item_style(placement);
        }
    }
}

fn finish_drag(ctx: &mut Context<'_, HomePage>, contact: ContactId, commit: bool) {
    let Some(drag) = ctx.me().drag.take_if(|drag| drag.contact == contact) else {
        return;
    };
    let accepted = commit && drag.preview.is_some();
    let placements = if accepted {
        drag.preview.unwrap()
    } else {
        drag.original
    };
    let items = ctx.me().items.clone();
    for (&item, &placement) in items.iter().zip(&placements) {
        ctx.widget_mut::<HomeItem>(item).unwrap().placement = placement;
    }
    ctx.me().occupancy = grid::occupancy(&placements).expect("drag result must be a valid grid");
    // This also replaces the dragged item's pixel offsets with percentages.
    apply_layout(ctx, &placements, None);
    tracing::info!(
        "Finished dragging item {}: committed={accepted}",
        drag.item_index
    );
}

/// Tree edits require App rather than Context. Keep the same item nodes and
/// vector indices, but draw the dragged subtree last (above its neighbours).
pub(super) struct RaiseItem {
    page: Handle<HomePage>,
    item: Handle<HomeItem>,
}
impl Signal for RaiseItem {}

pub(super) fn raise_item(app: &mut App, e: &RaiseItem) {
    if !app.is_live(e.page) || !app.is_live(e.item) {
        return;
    }
    if app.children(e.page).unwrap().last() == Some(&e.item.id()) {
        return;
    }
    // move_to(same_parent) is a no-op. A round trip through the page's parent
    // appends the item to the page. Both moves happen before layout/rendering.
    let parent = app.parent(e.page).unwrap();
    app.move_to(e.item, parent);
    app.move_to(e.item, e.page);
}

#[cfg(test)]
#[path = "../tests/unit/page.rs"]
mod tests;
