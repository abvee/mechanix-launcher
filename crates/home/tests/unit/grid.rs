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
        p(0, 3, 2, 1),
        p(3, 0, 1, 2),
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
    assert_eq!(candidate(footprint, 59.0, 99.0, size), footprint);
    assert_eq!(candidate(footprint, 60.0, 100.0, size), p(1, 1, 2, 2));
    assert_eq!(candidate(footprint, 1000.0, 1000.0, size), p(2, 2, 2, 2));
    assert_eq!(candidate(footprint, -100.0, -100.0, size), footprint);
}

#[test]
fn overlapping_neighbour_moves_to_nearest_free_anchor_with_stable_ties() {
    let original = [p(0, 0, 2, 2), p(3, 3, 1, 1), p(0, 3, 1, 1)];
    let preview = rearrange(&original, 0, p(2, 2, 2, 2)).unwrap();
    assert_eq!(preview, [p(2, 2, 2, 2), p(1, 3, 1, 1), original[2]]);
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
            .filter(|&icon| !overlaps(icon, mixed[0]))
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
    for row in 0..3 {
        for column in 0..3 {
            let large = p(row, column, 2, 2);
            for small_row in 0..4 {
                for small_column in 0..4 {
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
