//! Small, deterministic grid calculations. No widget state is changed here.

use crate::items::{HandleEdge, Placement};
use mecha_wayland::prelude::{Point, Size};

/// Row-major page grid: four columns and eight rows.
pub(super) const GRID_ROWS: usize = 8;
/// Column count of the same row-major page grid; see [`GRID_ROWS`].
pub(super) const GRID_COLUMNS: usize = 4;
/// Row-major cell ownership: `None` is empty; `Some(index)` names an item in
/// the owning page's `items` vector. A multi-cell item repeats its index
/// across its footprint.
pub(super) type Occupancy = [[Option<u8>; GRID_COLUMNS]; GRID_ROWS];

fn in_bounds(p: Placement) -> bool {
    p.width > 0
        && p.height > 0
        && p.width <= GRID_COLUMNS
        && p.height <= GRID_ROWS
        && p.column <= GRID_COLUMNS - p.width
        && p.row <= GRID_ROWS - p.height
}

fn fits(cells: &Occupancy, p: Placement) -> bool {
    in_bounds(p)
        && (p.row..p.row + p.height)
            .all(|row| (p.column..p.column + p.width).all(|column| cells[row][column].is_none()))
}

/// Caller has checked that the footprint fits.
fn occupy(cells: &mut Occupancy, index: usize, p: Placement) {
    for row in cells.iter_mut().skip(p.row).take(p.height) {
        for cell in row.iter_mut().skip(p.column).take(p.width) {
            *cell = Some(u8::try_from(index).expect("item index must fit in u8"));
        }
    }
}

/// Clear a valid item's entire footprint from the working occupancy.
fn clear(grid: &mut Occupancy, placement: Placement) {
    for row in placement.row..placement.row + placement.height {
        for column in placement.column..placement.column + placement.width {
            grid[row][column] = None;
        }
    }
}

/// Reject invalid spans, out-of-bounds footprints, and overlaps.
pub(super) fn occupancy(placements: &[Placement]) -> Option<Occupancy> {
    if placements.len() > u8::MAX as usize + 1 {
        return None;
    }
    let mut cells = [[None; GRID_COLUMNS]; GRID_ROWS];
    for (index, &placement) in placements.iter().enumerate() {
        if !fits(&cells, placement) {
            return None;
        }
        occupy(&mut cells, index, placement);
    }
    Some(cells)
}

/// Anchor nearest to the dragged top-left corner. The caller supplies a valid
/// footprint and positive page dimensions. Clamp the whole footprint inside.
pub(super) fn candidate(footprint: Placement, left: f32, top: f32, size: Size) -> Placement {
    Placement {
        column: (left / (size.width / GRID_COLUMNS as f32))
            .round()
            .clamp(0.0, (GRID_COLUMNS - footprint.width) as f32) as usize,
        row: (top / (size.height / GRID_ROWS as f32))
            .round()
            .clamp(0.0, (GRID_ROWS - footprint.height) as f32) as usize,
        ..footprint
    }
}

/// The footprint the dragged edge snaps to: the page-local point defines one
/// edge of the item, and the opposite edge stays pinned. Right and Bottom
/// change only the span; Left and Top also move the anchor. Boundaries snap
/// like `candidate` does — crossing at the halfway point, exact halves
/// rounding upward — and the clamps keep the result a valid footprint, so
/// this never fails.
pub(super) fn resize_candidate(
    footprint: Placement,
    edge: HandleEdge,
    point: Point,
    size: Size,
) -> Placement {
    let cell_width = size.width / GRID_COLUMNS as f32;
    let cell_height = size.height / GRID_ROWS as f32;

    match edge {
        // The pinned left edge keeps the column; the right boundary snaps.
        HandleEdge::Right => {
            let right = (point.x / cell_width)
                .round()
                .clamp(footprint.column as f32 + 1.0, GRID_COLUMNS as f32)
                as usize;
            Placement {
                width: right - footprint.column,
                ..footprint
            }
        }
        // The pinned top edge keeps the row; the bottom boundary snaps.
        HandleEdge::Bottom => {
            let bottom = (point.y / cell_height)
                .round()
                .clamp(footprint.row as f32 + 1.0, GRID_ROWS as f32)
                as usize;
            Placement {
                height: bottom - footprint.row,
                ..footprint
            }
        }
        // The right boundary stays pinned; the left edge snaps to it.
        HandleEdge::Left => {
            let pinned = footprint.column + footprint.width;
            let left = (point.x / cell_width)
                .round()
                .clamp(0.0, (pinned - 1) as f32) as usize;
            Placement {
                column: left,
                width: pinned - left,
                ..footprint
            }
        }
        // The bottom boundary stays pinned; the top edge snaps to it.
        HandleEdge::Top => {
            let pinned = footprint.row + footprint.height;
            let top = (point.y / cell_height)
                .round()
                .clamp(0.0, (pinned - 1) as f32) as usize;
            Placement {
                row: top,
                height: pinned - top,
                ..footprint
            }
        }
    }
}

/// Find displaced items through the candidate's occupied cells, leaving
/// unaffected items in place. Relocate in row-major first-encounter order
/// to the nearest free anchor (Manhattan
/// distance from their original anchor; ties use row, then column).
///
/// The candidate may differ from the dragged item's current size: a resize,
/// not just a move. Always start from the committed arrangement: a preview
/// never accumulates changes from earlier previews. This is a bounded greedy
/// search, not a puzzle solver: no recursive pushing or backtracking if an
/// item cannot fit.
pub(super) fn rearrange(
    original: &[Placement],
    dragged: usize,
    target: Placement,
) -> Option<Vec<Placement>> {
    // TODO: Reuse the committed occupancy captured at drag start instead of
    // rebuilding it on every candidate change. Each preview needs a working copy.
    let mut cells = occupancy(original)?;
    let source = *original.get(dragged)?;

    // Check if the target is a valid footprint, any size. clear() below
    // expects valid placements; occupancy(original) already validated the
    // source footprints.
    if !in_bounds(target) {
        return None;
    }

    let mut result = original.to_vec();
    result[dragged] = target;

    // clear the dragged item
    clear(&mut cells, source);

    let mut displaced = Vec::new();

    for row in target.row..target.row + target.height {
        for column in target.column..target.column + target.width {
            let Some(index) = cells[row][column] else {
                continue;
            };
            let index = usize::from(index);
            displaced.push(index);

            // Clear the item's whole footprint. Prevents rediscovery; we're going to
            // assign this again later anyways
            clear(&mut cells, original[index]);
        }
    }

    occupy(&mut cells, dragged, target);

    // Fill into new candidate slots and return.
    for index in displaced {
        let old = original[index];
        let mut nearest = None;
        let mut best_distance = usize::MAX;
        for row in 0..=GRID_ROWS - old.height {
            for column in 0..=GRID_COLUMNS - old.width {
                let p = Placement { row, column, ..old };

                // change algorithm here, it's nearest first
                let distance = row.abs_diff(old.row) + column.abs_diff(old.column);
                if distance < best_distance && fits(&cells, p) {
                    nearest = Some(p);
                    best_distance = distance;
                }
            }
        }
        let placement = nearest?;
        result[index] = placement;
        occupy(&mut cells, index, placement);
    }
    Some(result)
}

#[cfg(test)]
#[path = "../tests/unit/grid.rs"]
mod tests;
