//! Small, deterministic grid calculations. No widget state is changed here.

use crate::items::Placement;
use mecha_wayland::prelude::Size;

pub(super) const GRID_ROWS: usize = 4;
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

fn overlaps(a: Placement, b: Placement) -> bool {
    a.column < b.column + b.width
        && b.column < a.column + a.width
        && a.row < b.row + b.height
        && b.row < a.row + a.height
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

/// Reserve the dragged footprint and all unaffected items. Relocate directly
/// displaced items, largest first, to their nearest free anchor (Manhattan
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
    occupancy(original)?;
    let source = *original.get(dragged)?;
    if !in_bounds(target) || source.width != target.width || source.height != target.height {
        return None;
    }

    let mut result = original.to_vec();
    result[dragged] = target;
    let mut cells = [[None; GRID_COLUMNS]; GRID_ROWS];
    occupy(&mut cells, dragged, target);
    let mut displaced = Vec::new();
    for (index, &placement) in original.iter().enumerate() {
        if index == dragged {
            continue;
        }
        if overlaps(placement, target) {
            displaced.push(index);
        } else {
            occupy(&mut cells, index, placement);
        }
    }
    displaced.sort_by_key(|&index| {
        let p = original[index];
        (std::cmp::Reverse(p.width * p.height), index)
    });

    for index in displaced {
        let old = original[index];
        let mut nearest = None;
        let mut best_distance = usize::MAX;
        for row in 0..=GRID_ROWS - old.height {
            for column in 0..=GRID_COLUMNS - old.width {
                let p = Placement { row, column, ..old };
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
