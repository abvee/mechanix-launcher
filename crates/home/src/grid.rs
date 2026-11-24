//! Small, deterministic grid calculations. No widget state is changed here.

use crate::items::Placement;
use mecha_wayland::prelude::Size;

/// Row-major page grid: four columns and eight rows.
pub(super) const GRID_ROWS: usize = 8;
pub(super) const GRID_COLUMNS: usize = 4;
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

/// Find displaced items through the candidate's occupied cells, leaving
/// unaffected items in place. Relocate in row-major first-encounter order
/// to the nearest free anchor (Manhattan
/// distance from their original anchor; ties use row, then column).
///
/// Always start from the committed arrangement: a preview never accumulates
/// changes from earlier previews. This is a bounded greedy search, not a puzzle
/// solver: no recursive pushing or backtracking if an item cannot fit.
pub(super) fn rearrange(
    original: &[Placement],
    dragged: usize,
    target: Placement,
) -> Option<Vec<Placement>> {
    // TODO: Reuse the committed occupancy captured at drag start instead of
    // rebuilding it on every candidate change. Each preview needs a working copy.
    let mut cells = occupancy(original)?;
    let source = *original.get(dragged)?;

    // Check if the target is valid and keeps the dragged item's size.
    // Important cuz we will call clear() later and that expects valid
    // placements; occupancy(original) already validated the source footprints.
    if !in_bounds(target) || source.width != target.width || source.height != target.height {
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
