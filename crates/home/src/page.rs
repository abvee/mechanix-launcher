//! A full-size home page with percentage-positioned items and cell occupancy.
//!
//! Item placements are authoritative; occupancy maps each covered cell to an
//! index in the page's item vector. A centred SVG temporarily identifies the page.

use crate::items::*;
use mecha_wayland::prelude::*;
use std::time::Duration;

/// Longer side of the page icon, in logical pixels.
const LOGO_SIZE: u32 = 120;
/// Grid dimensions used for both percentage placement and occupancy indexing.
const GRID_ROWS: usize = 4;
const GRID_COLUMNS: usize = 4;
const LONG_PRESS_DELAY: Duration = Duration::from_millis(250);
const LONG_PRESS_SLOP: f32 = 5.0;

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

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum PageMode {
    Normal,
    Edit,
}

struct LongPress {
    contact: ContactId,
    /// Window-local position of the original press; deltas are measured from it.
    start_point: Point,
    /// Home successfully took ownership. Prevents repeated claims and logs.
    claimed: bool,
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

    long_press: Option<LongPress>, // ongoing long press to transition from normal mode to edit mode
    long_press_timer: TimerId, // ongoing transition timer. Should only be active if long_press is Some()
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
            long_press: None,
            long_press_timer,
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

    // we're already tracking another press that hasn't been released
    if page.long_press.is_some() {
        return;
    }

    page.long_press = Some(LongPress {
        contact: e.contact,
        start_point: e.position,
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
    clear_long_press(ctx, e.contact);
}

fn on_press_cancelled(ctx: &mut Context<'_, HomePage>, e: &PressCancelled) {
    clear_long_press(ctx, e.contact);
}

fn on_press_moved(ctx: &mut Context<'_, HomePage>, e: &PressMoved) {
    let page = ctx.me();

    let Some(press) = page.long_press.as_ref() else {
        return;
    };

    // skip when you already have a claimed press, we don't care if it moved
    // then. This happens when you long press then move with the samem motion
    if press.contact != e.contact || press.claimed {
        return;
    }

    let dx = e.position.x - press.start_point.x;
    let dy = e.position.y - press.start_point.y;

    // the old distance squared trick
    let distance_squared = dx * dx + dy * dy;
    if distance_squared > LONG_PRESS_SLOP * LONG_PRESS_SLOP {
        clear_long_press(ctx, e.contact);

        tracing::info!("Long press cancelled: {:?}", e.contact);
    }
}

fn on_long_press_timeout(ctx: &mut Context<'_, HomePage>, _: &Timeout) {
    let timer = ctx.me().long_press_timer;

    // ignore an older queued timeout if this timer has since restarted
    // This can happen if you long press, leave it before the timeout
    if ctx.timer_running(timer) {
        return;
    }

    let contact = {
        let page = ctx.me();

        let Some(press) = page.long_press.as_ref() else {
            return;
        };

        // one transition per press
        if press.claimed {
            return;
        }

        press.contact
    };

    // Claim the contact
    // This has to obviously happen before we change mode
    if !ctx.claim_contact(contact) {
        return;
    }

    let page = ctx.me();
    page.long_press.as_mut().unwrap().claimed = true;
    page.mode = match page.mode {
        PageMode::Normal => PageMode::Edit,
        PageMode::Edit => PageMode::Normal,
    };

    tracing::info!("Long press done, switched mode to: {:?}", page.mode);
}

#[cfg(test)]
#[path = "../tests/unit/page.rs"]
mod tests;
