use super::*;

fn p(row: usize, column: usize, width: usize, height: usize) -> Placement {
    Placement {
        row,
        column,
        width,
        height,
    }
}

#[test]
fn occupancy_rejects_invalid_footprints_and_collisions() {
    for placement in [
        p(0, 0, 0, 1),
        p(0, 0, 1, 0),
        p(0, GRID_COLUMNS - 1, 2, 1),
        p(GRID_ROWS - 1, 0, 1, 2),
        p(usize::MAX, 0, 1, 1),
        p(0, 0, usize::MAX, 1),
    ] {
        assert!(occupancy(&[placement]).is_none());
    }
    assert!(occupancy(&[p(0, 0, 2, 2), p(1, 1, 1, 1)]).is_none());
    assert!(occupancy(&[p(0, 0, 2, 2), p(2, 2, 2, 2)]).is_some());
}

#[test]
fn candidate_rounds_at_half_a_cell_and_clamps_the_whole_item() {
    let footprint = p(0, 0, 2, 2);
    let size = Size::new(480.0, 800.0);
    let half_cell_width = size.width / GRID_COLUMNS as f32 / 2.0;
    let half_cell_height = size.height / GRID_ROWS as f32 / 2.0;
    assert_eq!(
        candidate(
            footprint,
            half_cell_width - 1.0,
            half_cell_height - 1.0,
            size
        ),
        footprint
    );
    assert_eq!(
        candidate(footprint, half_cell_width, half_cell_height, size),
        p(1, 1, 2, 2)
    );
    assert_eq!(
        candidate(footprint, 1000.0, 1000.0, size),
        p(GRID_ROWS - 2, GRID_COLUMNS - 2, 2, 2)
    );
    assert_eq!(candidate(footprint, -100.0, -100.0, size), footprint);
}

#[test]
fn overlapping_neighbour_moves_to_nearest_free_anchor_with_stable_ties() {
    let original = [p(5, 0, 2, 2), p(1, 1, 1, 1), p(7, 3, 1, 1)];
    let preview = rearrange(&original, 0, p(0, 0, 2, 2)).unwrap();
    // (row 1, column 2) and (row 2, column 1) are equally near.
    assert_eq!(preview, [p(0, 0, 2, 2), p(1, 2, 1, 1), original[2]]);
    assert!(occupancy(&preview).is_some());
    // Returning to the original anchor exactly restores the original arrangement.
    assert_eq!(rearrange(&original, 0, original[0]).unwrap(), original);
}

#[test]
fn full_grid_can_swap_icons_but_rejects_fragmented_space_for_a_large_widget() {
    let icons: Vec<_> = (0..GRID_ROWS)
        .flat_map(|row| (0..GRID_COLUMNS).map(move |column| p(row, column, 1, 1)))
        .collect();
    let preview = rearrange(&icons, 0, icons[1]).unwrap();
    assert_eq!(preview[0], icons[1]);
    assert_eq!(preview[1], icons[0]);
    assert!(occupancy(&preview).is_some());

    let mut mixed = vec![p(0, 0, 2, 2)];
    mixed.extend(
        icons
            .into_iter()
            .filter(|icon| icon.row >= 2 || icon.column >= 2)
            .collect::<Vec<_>>(),
    );
    let dragged = mixed.len() - 1;
    assert!(occupancy(&mixed).is_some());
    assert!(rearrange(&mixed, dragged, p(0, 0, 1, 1)).is_none());
    assert!(rearrange(&mixed, dragged, p(0, 0, 2, 1)).is_none());
}

#[test]
fn every_two_item_move_keeps_valid_sizes_and_non_overlapping_placements() {
    // Exhaust all legal starting layouts and targets for a 2x2 widget + icon.
    for row in 0..=GRID_ROWS - 2 {
        for column in 0..=GRID_COLUMNS - 2 {
            let large = p(row, column, 2, 2);
            for small_row in 0..GRID_ROWS {
                for small_column in 0..GRID_COLUMNS {
                    let original = [large, p(small_row, small_column, 1, 1)];
                    if occupancy(&original).is_none() {
                        continue;
                    }
                    for dragged in 0..2 {
                        for target_row in 0..=GRID_ROWS - original[dragged].height {
                            for target_column in 0..=GRID_COLUMNS - original[dragged].width {
                                let target = Placement {
                                    row: target_row,
                                    column: target_column,
                                    ..original[dragged]
                                };
                                let preview = rearrange(&original, dragged, target).unwrap();
                                assert!(occupancy(&preview).is_some());
                                assert_eq!(preview[dragged], target);
                                for index in 0..2 {
                                    assert_eq!(preview[index].width, original[index].width);
                                    assert_eq!(preview[index].height, original[index].height);
                                }
                                assert_eq!(rearrange(&original, dragged, target).unwrap(), preview);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn clear_rectangular_footprints_preserves_neighbours() {
    for footprint in [p(2, 1, 2, 1), p(2, 1, 1, 3)] {
        let mut cells = [[Some(9); GRID_COLUMNS]; GRID_ROWS];
        clear(&mut cells, footprint);
        let empty: Vec<_> = cells
            .iter()
            .enumerate()
            .flat_map(|(row, cells)| {
                cells.iter().enumerate().filter_map(move |(column, cell)| {
                    if cell.is_none() {
                        Some((row, column))
                    } else {
                        assert_eq!(*cell, Some(9));
                        None
                    }
                })
            })
            .collect();
        let expected = if footprint.width == 2 {
            vec![(2, 1), (2, 2)]
        } else {
            vec![(2, 1), (3, 1), (4, 1)]
        };
        assert_eq!(empty, expected);
    }
}

#[test]
fn multi_cell_collision_clears_the_whole_neighbour_and_preserves_other_items() {
    let original = [p(0, 0, 2, 2), p(2, 2, 2, 1), p(7, 0, 2, 1)];
    // The target intersects both cells of item 1. It must be relocated once.
    let preview = rearrange(&original, 0, p(1, 2, 2, 2)).unwrap();
    assert_eq!(preview, [p(1, 2, 2, 2), p(3, 2, 2, 1), original[2]]);
    assert!(occupancy(&preview).is_some());

    // Now intersect only one of item 1's cells. Its old cell outside the target
    // must also be freed, making (2, 2) available for the moved 2x1 neighbour.
    let original = [p(0, 0, 1, 1), p(2, 1, 2, 1), p(1, 1, 2, 1)];
    let preview = rearrange(&original, 0, p(2, 1, 1, 1)).unwrap();
    assert_eq!(preview, [p(2, 1, 1, 1), p(2, 2, 2, 1), original[2]]);
    assert!(occupancy(&preview).is_some());
}

#[test]
fn resize_candidate_snaps_each_edge_and_pins_its_opposite() {
    // Spans columns 1..3 and rows 2..4: boundaries 1|3 horizontally, 2|4
    // vertically. Cells are 120x100 at this page size.
    let footprint = p(2, 1, 2, 2);
    let size = Size::new(480.0, 800.0);

    // Right: column and rows stay; the boundary snaps at halfway (420).
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Right, Point::new(419.0, 0.0), size),
        footprint
    );
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Right, Point::new(420.0, 0.0), size),
        p(2, 1, 3, 2)
    );

    // Bottom: halfway between 400 and 500 is 450.
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Bottom, Point::new(0.0, 449.0), size),
        footprint
    );
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Bottom, Point::new(0.0, 450.0), size),
        p(2, 1, 2, 3)
    );

    // Left: the right boundary stays pinned at 3; growing crosses below 60,
    // and shrinking past 180 cuts a column off the left.
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Left, Point::new(61.0, 0.0), size),
        footprint
    );
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Left, Point::new(59.0, 0.0), size),
        p(2, 0, 3, 2)
    );
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Left, Point::new(180.0, 0.0), size),
        p(2, 2, 1, 2)
    );

    // Top: the bottom boundary stays pinned at 4; growing crosses above
    // halfway (150), shrinking past 250 cuts a row off the top.
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Top, Point::new(0.0, 151.0), size),
        footprint
    );
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Top, Point::new(0.0, 149.0), size),
        p(1, 1, 2, 3)
    );
    assert_eq!(
        resize_candidate(footprint, HandleEdge::Top, Point::new(0.0, 250.0), size),
        p(3, 1, 2, 1)
    );
}

#[test]
fn resize_candidate_clamps_to_the_grid_and_a_one_cell_minimum() {
    // p(0,0,2,2) in the top-left corner; p(6,2,2,2) in the bottom-right.
    let corner = p(0, 0, 2, 2);
    let floor = p(6, 2, 2, 2);
    let size = Size::new(480.0, 800.0);

    // Right never leaves the grid or drops below one column.
    assert_eq!(
        resize_candidate(floor, HandleEdge::Right, Point::new(1000.0, 0.0), size),
        floor
    );
    assert_eq!(
        resize_candidate(corner, HandleEdge::Right, Point::new(-1000.0, 0.0), size),
        p(0, 0, 1, 2)
    );

    // Bottom never leaves the grid or drops below one row.
    assert_eq!(
        resize_candidate(floor, HandleEdge::Bottom, Point::new(0.0, 2000.0), size),
        floor
    );
    assert_eq!(
        resize_candidate(corner, HandleEdge::Bottom, Point::new(0.0, -1000.0), size),
        p(0, 0, 2, 1)
    );

    // Left never crosses the grid's left edge or the pinned right boundary.
    assert_eq!(
        resize_candidate(corner, HandleEdge::Left, Point::new(-1000.0, 0.0), size),
        corner
    );
    assert_eq!(
        resize_candidate(floor, HandleEdge::Left, Point::new(1000.0, 0.0), size),
        p(6, 3, 1, 2)
    );

    // Top never crosses the grid's top edge or the pinned bottom boundary.
    assert_eq!(
        resize_candidate(corner, HandleEdge::Top, Point::new(0.0, -1000.0), size),
        corner
    );
    assert_eq!(
        resize_candidate(floor, HandleEdge::Top, Point::new(0.0, 2000.0), size),
        p(7, 2, 2, 1)
    );
}

#[test]
fn rearrange_accepts_a_resize_and_relocates_only_the_displaced() {
    // Grow item 0 rightward and downward over the icon at (1, 3).
    let original = [p(0, 0, 2, 2), p(1, 3, 1, 1)];
    let target = p(0, 0, 4, 3);
    let preview = rearrange(&original, 0, target).unwrap();
    // The icon's nearest free home is (3, 3): everything nearer is covered.
    assert_eq!(preview, [target, p(3, 3, 1, 1)]);
    assert!(occupancy(&preview).is_some());
}

#[test]
fn rearrange_rejects_a_resize_with_nowhere_to_go_but_always_fits_a_shrink() {
    // Every cell occupied: the icons under a grown footprint have no free
    // anchor anywhere, so the resize is rejected outright. A shrink frees
    // cells and can displace nobody, so it always fits.
    let mut placements = vec![p(0, 0, 2, 2)];
    for row in 0..GRID_ROWS {
        for column in 0..GRID_COLUMNS {
            // Skip the 2x2's own cells (rows 0..2, columns 0..2).
            if row < 2 && column < 2 {
                continue;
            }
            placements.push(p(row, column, 1, 1));
        }
    }
    assert!(occupancy(&placements).is_some());

    assert!(rearrange(&placements, 0, p(0, 0, 3, 2)).is_none());

    let preview = rearrange(&placements, 0, p(0, 0, 1, 1)).unwrap();
    assert_eq!(preview[0], p(0, 0, 1, 1));
    assert!(occupancy(&preview).is_some());
}
