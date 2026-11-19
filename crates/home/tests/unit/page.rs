//! Initial page occupancy, checked against the actual spawned item nodes.
//! Included under `page` so these tests can inspect its private bookkeeping.

use super::*;
use crate::HomeScreen;
use screen::Screen;

#[test]
fn initial_occupancy_matches_each_pages_spawned_items() {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(InteractivityModule)
        .add_module(MechanixTheme::dark())
        .insert_resource(Atlas::new());

    let root = app.root();
    let window = app
        .spawn(
            root,
            window().layout(LayoutStyle::default().size(px(480.0), px(800.0))),
        )
        .id();
    HomeScreen.build(&mut app, window);
    app.tick();

    // Item 0 spans the upper-left 2×2 cells; item 1 occupies the bottom-right.
    // All other cells must remain empty.
    let expected = [
        [Some(0), Some(0), None, None],
        [Some(0), Some(0), None, None],
        [None, None, None, None],
        [None, None, None, Some(1)],
    ];
    let placements = [(0, 0, 2, 2), (3, 3, 1, 1)];

    let pages: Vec<_> = app.widgets::<HomePage>().map(|(id, _)| id).collect();
    assert_eq!(pages.len(), 4);
    for page_id in pages {
        let page = app.widget::<HomePage>(page_id).unwrap();
        assert_eq!(page.occupancy, expected, "page {page_id:?}");
        assert_eq!(page.items.len(), placements.len());

        let children = app.children(page_id).unwrap();
        let spawned_items: Vec<_> = children
            .iter()
            .copied()
            .filter(|&id| app.widget::<HomeItem>(id).is_some())
            .collect();
        let tracked_items: Vec<_> = page.items.iter().map(|item| item.id()).collect();
        assert_eq!(
            spawned_items, tracked_items,
            "track each item in spawn order"
        );

        // The table's indices must refer to live items with these footprints,
        // not merely contain the right numbers independently of the vector.
        for (index, expected_placement) in placements.iter().enumerate() {
            let placement = &app.widget::<HomeItem>(page.items[index].id()).unwrap().0;
            assert_eq!(
                (
                    placement.row,
                    placement.column,
                    placement.width,
                    placement.height,
                ),
                *expected_placement,
            );
        }
    }
}
