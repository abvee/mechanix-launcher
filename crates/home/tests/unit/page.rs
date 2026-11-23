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
        .init_resource::<Timers>()
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

/// Full input routing, with a page offset from the window origin. Timers are
/// completed synthetically so these tests need neither sleeping nor io_uring.
struct Ui {
    app: App,
    window: NodeId,
    home: NodeId,
    page: NodeId,
}

struct ExpireTimer(TimerId);
impl Event for ExpireTimer {}

struct TestClock(NodeId);
impl Build for TestClock {
    type Widget = Self;
}
impl Widget for TestClock {
    type Builder = Self;
    fn build(b: Self, _: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        s.on::<ExpireTimer>(b.0, |ctx, e| {
            // Match a one-shot expiry: stop countdown before delivering Timeout.
            ctx.cancel_timer(e.0);
            ctx.emit(Timeout { id: e.0 }, ctx.target());
        });
        b
    }
}

impl Ui {
    fn new() -> Self {
        let mut app = App::new();
        app.add_module(LayoutModule)
            .add_module(PaintModule)
            .add_module(WindowModule)
            .add_module(InteractivityModule)
            .add_module(MechanixTheme::dark())
            .init_resource::<Timers>()
            .insert_resource(Atlas::new());
        let root = app.root();
        let window = app
            .spawn(
                root,
                window().layout(LayoutStyle::default().size(px(560.0), px(920.0))),
            )
            .id();
        let home = HomeScreen.build(&mut app, window);
        *app.component_mut::<LayoutStyle>(home).unwrap() = LayoutStyle::default()
            .absolute()
            .inset(Insets::new(px(60.0), auto(), auto(), px(40.0)))
            .size(px(480.0), px(800.0));
        let page = app.widget::<crate::Home>(home).unwrap().pages[0].id();
        app.spawn(root, TestClock(page));
        app.tick();
        Self {
            app,
            window,
            home,
            page,
        }
    }

    fn page(&self) -> &HomePage {
        self.app.widget::<HomePage>(self.page).unwrap()
    }

    fn input(&mut self, finger: u32, phase: ContactPhase, x: f32, y: f32) {
        self.app.signal(ContactInput {
            window: self.window,
            contact: ContactId::Touch(finger),
            phase,
            position: Point::new(x + 40.0, y + 60.0),
            time: 0,
        });
        self.app.tick();
    }

    fn press(&mut self, x: f32, y: f32) {
        self.input(0, ContactPhase::Moved, x, y);
        self.input(0, ContactPhase::Pressed, x, y);
    }

    fn timeout(&mut self) {
        let id = self.page().long_press_timer;
        self.app.emit(ExpireTimer(id), self.page);
        self.app.tick();
    }

    fn placements(&self) -> Vec<Placement> {
        self.page()
            .items
            .iter()
            .map(|&item| self.app.widget::<HomeItem>(item).unwrap().0)
            .collect()
    }

    fn rect(&self, index: usize) -> Rect {
        self.app
            .component::<ComputedLayout>(self.page().items[index])
            .unwrap()
            .rect
    }

    fn assert_grid_layout(&self, placements: &[Placement]) {
        assert_eq!(self.placements(), placements);
        assert_eq!(self.page().occupancy, grid::occupancy(placements).unwrap());
        for (index, placement) in placements.iter().enumerate() {
            assert_eq!(
                *self
                    .app
                    .component::<LayoutStyle>(self.page().items[index])
                    .unwrap(),
                item_style(placement),
            );
            let rect = self.rect(index);
            assert_eq!(rect.x(), 40.0 + placement.column as f32 * 120.0);
            assert_eq!(rect.y(), 60.0 + placement.row as f32 * 200.0);
        }
    }
}

#[test]
fn long_press_starts_drag_without_lifting_and_live_preview_commits_on_release() {
    let mut ui = Ui::new();
    let original = ui.placements();
    let occupancy = ui.page().occupancy;
    let item = ui.page().items[0];
    ui.press(30.0, 40.0);
    ui.input(0, ContactPhase::Moved, 33.0, 44.0); // exactly five pixels of slop
    ui.timeout();
    assert_eq!(ui.page().mode, PageMode::Edit);
    assert!(ui.page().long_press.is_none());
    assert!(ui.page().drag.is_some());
    assert!(
        ui.app
            .widget::<crate::Home>(ui.home)
            .unwrap()
            .drag
            .is_none()
    );
    assert_eq!(ui.app.children(ui.page).unwrap().last(), Some(&item.id()));
    assert_eq!((ui.rect(0).x(), ui.rect(0).y()), (43.0, 64.0));

    ui.input(0, ContactPhase::Moved, 270.0, 440.0);
    let preview = ui.page().drag.as_ref().unwrap().preview.clone().unwrap();
    assert_eq!((preview[0].column, preview[0].row), (2, 2));
    assert_eq!((preview[1].column, preview[1].row), (3, 1));
    assert_eq!((ui.rect(0).x(), ui.rect(0).y()), (280.0, 460.0));
    // The neighbour has visibly moved BEFORE release.
    assert_eq!((ui.rect(1).x(), ui.rect(1).y()), (400.0, 260.0));
    assert_eq!(ui.placements(), original);
    assert_eq!(ui.page().occupancy, occupancy);

    ui.input(0, ContactPhase::Released, 270.0, 440.0);
    assert!(ui.page().drag.is_none());
    assert_eq!(ui.page().mode, PageMode::Edit);
    ui.assert_grid_layout(&preview);
    assert_eq!(
        ui.app.widget::<crate::Home>(ui.home).unwrap().active_page,
        0
    );
}

#[test]
fn edit_mode_grabs_immediately_and_cancel_restores_neighbours_and_dragged_item() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    let original = ui.placements();
    ui.press(30.0, 40.0);
    assert!(ui.page().drag.is_some());
    assert!(
        !ui.app
            .resource::<Timers>()
            .is_running(ui.page().long_press_timer)
    );
    ui.input(0, ContactPhase::Moved, 270.0, 440.0);
    assert_ne!(ui.rect(1).y(), 660.0);

    // An unrelated release/cancellation must not finish the drag.
    ui.app.emit(
        Release {
            contact: ContactId::Touch(1),
            position: Point::new(0.0, 0.0),
            time: 0,
        },
        ui.page,
    );
    ui.app.emit(
        PressCancelled {
            contact: ContactId::Touch(1),
            position: Point::new(0.0, 0.0),
        },
        ui.page,
    );
    ui.app.tick();
    assert!(ui.page().drag.is_some());
    ui.input(0, ContactPhase::Cancelled, 270.0, 440.0);
    assert!(ui.page().drag.is_none());
    assert_eq!(ui.page().mode, PageMode::Edit);
    ui.assert_grid_layout(&original);
}

#[test]
fn movement_within_one_candidate_still_follows_finger_and_return_restores_preview() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    let original = ui.placements();
    ui.press(30.0, 40.0);
    ui.input(0, ContactPhase::Moved, 220.0, 390.0);
    let preview = ui.page().drag.as_ref().unwrap().preview.clone();
    ui.input(0, ContactPhase::Moved, 230.0, 400.0);
    assert_eq!(ui.page().drag.as_ref().unwrap().preview, preview);
    assert_eq!((ui.rect(0).x(), ui.rect(0).y()), (240.0, 420.0));
    ui.input(0, ContactPhase::Moved, 30.0, 40.0);
    assert_eq!(
        ui.page().drag.as_ref().unwrap().preview.as_ref().unwrap(),
        &original
    );
    assert_eq!(ui.rect(1).y(), 660.0);
    ui.input(0, ContactPhase::Released, 30.0, 40.0);
    ui.assert_grid_layout(&original);
}

#[test]
fn release_uses_its_final_position_and_clamps_whole_footprint() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    ui.press(30.0, 40.0);
    ui.input(0, ContactPhase::Released, 2000.0, 2000.0);
    let placements = ui.placements();
    assert_eq!((placements[0].row, placements[0].column), (2, 2));
    ui.assert_grid_layout(&placements);
}

#[test]
fn invalid_drop_rolls_back_a_packed_grid() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    // Fill every remaining cell with icons. Moving the bottom-right icon into
    // the 2x2 widget leaves no contiguous 2x2 destination for that widget.
    for row in 0..GRID_ROWS {
        for column in 0..GRID_COLUMNS {
            if ui.page().occupancy[row][column].is_some() {
                continue;
            }
            let placement = Placement {
                row,
                column,
                width: 1,
                height: 1,
            };
            let item = ui.app.spawn(ui.page, HomeItem(placement));
            *ui.app.component_mut::<LayoutStyle>(item).unwrap() = item_style(&placement);
            ui.app
                .widget_mut::<HomePage>(ui.page)
                .unwrap()
                .items
                .push(item);
        }
    }
    let original = ui.placements();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().occupancy = grid::occupancy(&original).unwrap();
    ui.app.tick();
    ui.press(390.0, 640.0);
    ui.input(0, ContactPhase::Moved, 270.0, 640.0);
    assert!(ui.page().drag.as_ref().unwrap().preview.is_some());
    ui.input(0, ContactPhase::Moved, 30.0, 40.0);
    assert!(ui.page().drag.as_ref().unwrap().preview.is_none());
    assert_eq!((ui.rect(1).x(), ui.rect(1).y()), (40.0, 60.0));
    for (index, placement) in original.iter().enumerate() {
        if index != 1 {
            assert_eq!(
                *ui.app
                    .component::<LayoutStyle>(ui.page().items[index])
                    .unwrap(),
                item_style(placement),
            );
        }
    }
    ui.input(0, ContactPhase::Released, 30.0, 40.0);
    ui.assert_grid_layout(&original);
}

#[test]
fn empty_space_long_press_toggles_mode_without_dragging() {
    let mut ui = Ui::new();
    for expected in [PageMode::Edit, PageMode::Normal] {
        ui.press(300.0, 100.0);
        ui.timeout();
        assert_eq!(ui.page().mode, expected);
        assert!(ui.page().drag.is_none());
        // Even a duplicate timeout may only toggle once.
        ui.timeout();
        assert_eq!(ui.page().mode, expected);
        ui.input(0, ContactPhase::Released, 300.0, 100.0);
    }
}

#[test]
fn release_cancellation_and_excess_slop_invalidate_pending_long_press() {
    for phase in [
        ContactPhase::Released,
        ContactPhase::Cancelled,
        ContactPhase::Moved,
    ] {
        let mut ui = Ui::new();
        ui.press(30.0, 40.0);
        ui.input(0, phase, 36.0, 40.0);
        assert!(ui.page().long_press.is_none());
        assert!(
            !ui.app
                .resource::<Timers>()
                .is_running(ui.page().long_press_timer)
        );
        ui.timeout();
        assert_eq!(ui.page().mode, PageMode::Normal);
        assert!(ui.page().drag.is_none());
    }
}

#[test]
fn stale_timeout_during_restarted_countdown_is_ignored() {
    let mut ui = Ui::new();
    ui.press(30.0, 40.0);
    ui.input(0, ContactPhase::Released, 30.0, 40.0);
    ui.press(30.0, 40.0);
    let id = ui.page().long_press_timer;
    ui.app.emit(Timeout { id }, ui.page);
    ui.app.tick();
    assert_eq!(ui.page().mode, PageMode::Normal);
    assert!(!ui.page().long_press.as_ref().unwrap().claimed);
    ui.timeout();
    assert_eq!(ui.page().mode, PageMode::Edit);
    assert!(ui.page().drag.is_some());
}
