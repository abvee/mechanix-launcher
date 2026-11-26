//! A full-size home page with percentage-positioned items and cell occupancy.
//!
//! Item placements are authoritative; occupancy maps each covered cell to an
//! index in the page's item vector. A centred SVG temporarily identifies the page.
//! A drag changes wrapper styles immediately, but only commits placements and
//! occupancy on release. Cancellation restores the original arrangement.
//! A long press toggles Edit mode, where a press on an item grabs it directly
//! and each item's × button requests its removal.

use crate::grid::{self, GRID_COLUMNS, GRID_ROWS, Occupancy};
use crate::items::*;
use mecha_wayland::prelude::*;
use std::time::Duration;

/// Longer side of the page icon, in logical pixels.
const LOGO_SIZE: u32 = 120;
/// How long a press must stay within the slop before it toggles the page's mode.
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

/// How the page interprets presses. A long press toggles between them;
/// each mode handles the same state differently.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub(super) enum PageMode {
    /// Taps pass through to children; a long press enters Edit.
    Normal,
    /// A press on an item's content grabs it directly; the × button is shown.
    Edit,
    /// Only resize handles do anything; the × is hidden. The debug button
    /// toggles in and out until the real gesture exists.
    Resize,
}

/// Emit to HomeItems whenever we change modes.
pub(super) struct HomePageModeChanged(pub(super) PageMode);
impl Event for HomePageModeChanged {}

/// A pending or recognized mode-toggle press.
struct LongPress {
    /// The contact being tracked.
    contact: ContactId,
    /// Window-local position of the original press; deltas are measured from it.
    start_point: Point,
    /// Latest position within the slop, so starting a drag follows the finger.
    position: Point,
    /// The page successfully took ownership. Prevents repeated mode toggles.
    claimed: bool,
}

/// An item drag in progress. Previews are not committed until release.
struct ItemDrag {
    /// The finger driving the drag.
    contact: ContactId,
    /// Index into `items` of the dragged wrapper.
    item_index: usize,
    /// Where the finger grabbed the wrapper, relative to its top-left corner.
    grab_offset: Point,
    /// Committed placements when the drag started; the rollback target.
    original: Vec<Placement>,
    /// Grid anchor nearest the finger; rearrangement runs only when it changes.
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
    /// Current mode. A long press toggles it; press handling differs per mode.
    mode: PageMode,
    /// Pending or recognized mode-toggle press; separate from item dragging.
    long_press: Option<LongPress>,
    /// One-shot timer restarted for each eligible press; runs only while
    /// `long_press` is `Some`.
    long_press_timer: TimerId,
    /// The item drag in progress, if any.
    drag: Option<ItemDrag>,
    /// Debug-only toggle into Resize mode; `None` on real pages. Read by
    /// the tests only, hence the allow.
    // TODO: REMOVE with the real resize entry gesture.
    #[allow(dead_code)]
    debug_button: Option<Handle<Div>>,
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
        // input handling
        s.on::<Press>(me, on_press)
            .on::<Release>(me, on_release)
            .on::<PressMoved>(me, on_press_moved)
            .on::<PressCancelled>(me, on_press_cancelled)
            .on::<RemoveItemRequest>(me, on_remove_item);

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

        // TODO: REMOVE. Debug entry into Resize mode until the gesture is decided.
        let debug_button = builder.debug_items.then(|| spawn_debug_button(me, s));

        s.on::<ResizeItemRequest>(me, |_ctx, e| {
            tracing::info!(
                "Resize requested for item: {:?} on edge: {:?}",
                e.item.id(),
                e.edge
            )
        });
        HomePage {
            items,
            occupancy,
            mode: PageMode::Normal,
            long_press: None,
            long_press_timer,
            drag: None,
            debug_button,
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

/// In Edit mode, a press on an item's content claims the contact and starts
/// a drag. Otherwise, track a possible long-press mode toggle.
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

/// Finish any drag at the release position (committing its preview), then
/// clear pending long-press state.
fn on_release(ctx: &mut Context<'_, HomePage>, e: &Release) {
    // A release can carry a newer position than the last movement event.
    move_drag(ctx, e.contact, e.position);
    finish_drag(ctx, e.contact, true);
    clear_long_press(ctx, e.contact);
}

/// Roll back any drag, then clear pending long-press state.
fn on_press_cancelled(ctx: &mut Context<'_, HomePage>, e: &PressCancelled) {
    finish_drag(ctx, e.contact, false);
    clear_long_press(ctx, e.contact);
}

/// Move an active drag. Otherwise, cancel a pending long press that moved
/// beyond the slop; a recognized (claimed) press is left alone.
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

// helper function: change the page's mode and notify items
fn set_mode(ctx: &mut Context<'_, HomePage>, mode: PageMode) {
    let page = ctx.me();
    if page.mode == mode {
        return;
    }
    page.mode = mode;
    let targets: Vec<NodeId> = page.items.iter().map(|&item| item.id()).collect();
    tracing::info!("Switching to mode: {:?}", mode);

    ctx.emit(HomePageModeChanged(mode), targets);
}

/// Long-press recognition: claim the contact, toggle the mode, notify the
/// items, and — when entering Edit — start dragging the pressed item.
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

    let mode = {
        let page = ctx.me();
        page.long_press.as_mut().unwrap().claimed = true;
        match page.mode {
            PageMode::Normal => PageMode::Edit,
            PageMode::Edit => PageMode::Normal,
            // placeholder decision. Real gesture decision comes later
            PageMode::Resize => PageMode::Normal,
        }
    };

    set_mode(ctx, mode);

    // you should be able to start dragging in the same long press
    if mode == PageMode::Edit {
        if let Some(index) = item_at(ctx, start_point) {
            start_drag(ctx, contact, index, start_point, position);
        }
    }
}

/// Find the item whose draggable content contains the window-local press.
/// Scan once in item order, stopping at the first match.
fn item_at(ctx: &mut Context<'_, HomePage>, point: Point) -> Option<usize> {
    let items = ctx.me().items.clone();
    items
        .iter()
        .position(|&item| HomeItem::can_drag_at(&mut ctx.at(item).unwrap(), point))
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

/// Move the dragged wrapper to follow the finger. Recompute the candidate
/// anchor and, when it changes, apply the rearrangement preview to the
/// other wrappers. Nothing is committed here.
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

/// End the matching drag: commit its preview on release, or restore the
/// original arrangement on cancellation or an invalid drop.
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

/// Remove the requesting item: drop it from the list and the tree, then
/// rebuild occupancy, whose indices shifted. Refused while a drag is
/// running, and a no-op for an item that is already gone.
fn on_remove_item(ctx: &mut Context<'_, HomePage>, e: &RemoveItemRequest) {
    // an on going drag relies on indicies being stable. Don't remove while a drag is on going
    // This could happen if you're dragging with one finger and use another
    // finger to hit the X button
    let page = ctx.me();
    if page.drag.is_some() {
        return;
    }

    // find handle. There could be a scheduling issue, so this might already
    // have been removed, hence the let Some()
    let Some(index) = page.items.iter().position(|&item| item == e.0) else {
        return;
    };

    ctx.me().items.remove(index);
    ctx.remove(e.0);
    tracing::info!("Removed item {index}");

    // rebuild occupancy
    let items: Vec<_> = ctx.me().items.clone();
    let placements: Vec<Placement> = items
        .iter()
        .map(|&item| ctx.widget::<HomeItem>(item).unwrap().placement)
        .collect();

    ctx.me().occupancy = grid::occupancy(&placements).expect("removal keeps a valid grid");
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

// ── debug: resize-mode entry ─────────────────────────────────────────
// TODO: REMOVE. Temporary stand-in for the real resize-mode entry gesture,
// so the resize pipeline can be built and tested ahead of the gesture
// design. Only spawns on debug pages.

/// Edge of the debug button, in logical pixels.
const DEBUG_BUTTON_SIZE: f32 = 32.0;

/// Spawn the debug button: a red square in the page's bottom-left corner
/// whose click toggles Resize mode. TODO: REMOVE with the real gesture.
fn spawn_debug_button(me: Handle<HomePage>, s: &mut Spawner<'_, HomePage>) -> Handle<Div> {
    let button = s.spawn(
        me,
        div().style(
            LayoutStyle::default()
                .absolute()
                .inset(Insets::new(auto(), auto(), px(0.0), px(0.0)))
                .size(px(DEBUG_BUTTON_SIZE), px(DEBUG_BUTTON_SIZE)),
        ),
    );
    // Error colour so it is unmistakably not a real widget.
    *s.component_mut::<Paint>(button).unwrap() = Paint::Quad(Quad::new(s.color(ColorRole::Error)));

    s.on::<Press>(button, |ctx, e| {
        // Claim like the ×: holding this button must never become a page
        // long-press that flips modes underneath us.
        ctx.claim_contact(e.contact);
    })
    .on::<Clicked>(button, |ctx, _| {
        // Toggle into Resize and back out; set_mode notifies the items.
        let mode = if ctx.me().mode == PageMode::Resize {
            PageMode::Normal
        } else {
            PageMode::Resize
        };
        set_mode(ctx, mode);
    });

    button
}

#[cfg(test)]
#[path = "../tests/unit/page.rs"]
mod tests;
