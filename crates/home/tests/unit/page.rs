//! Page occupancy, gesture routing, and live layout previews.
//! Included under `page` so these tests can inspect its private bookkeeping.

use super::*;
use crate::HomeScreen;
use mecha_wayland::mechanix_widgets::prelude::FontBook;
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
    let font = app
        .resource_mut::<Atlas>()
        .add_font(assets::fonts::GEIST_MONO)
        .unwrap();
    app.insert_resource(FontBook::new(font));

    let root = app.root();
    let window = app
        .spawn(
            root,
            window().layout(LayoutStyle::default().size(px(480.0), px(800.0))),
        )
        .id();
    let home = HomeScreen.build(&mut app, window);
    app.tick();

    // Only the last page has debug items: a 2x2, a 1x1, and a bottom-row 2x1.
    // All other cells, including those on the three empty pages, remain empty.
    let mut expected = [[None; GRID_COLUMNS]; GRID_ROWS];
    expected[0][0] = Some(0);
    expected[0][1] = Some(0);
    expected[1][0] = Some(0);
    expected[1][1] = Some(0);
    expected[3][3] = Some(1);
    expected[7][0] = Some(2);
    expected[7][1] = Some(2);
    let placements = [(0, 0, 2, 2), (3, 3, 1, 1), (7, 0, 2, 1)];

    let pages = app.widget::<crate::Home>(home).unwrap().pages.clone();
    assert_eq!(pages.len(), 4);
    for (page_index, page_id) in pages.into_iter().enumerate() {
        let page = app.widget::<HomePage>(page_id).unwrap();
        if page_index < 3 {
            assert!(page.items.is_empty());
            assert_eq!(page.occupancy, [[None; GRID_COLUMNS]; GRID_ROWS]);
            assert!(
                !app.descendants(page_id)
                    .any(|id| app.widget::<HomeItem>(id).is_some())
            );
            continue;
        }
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
            let placement = &app
                .widget::<HomeItem>(page.items[index].id())
                .unwrap()
                .placement;
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
        let font = app
            .resource_mut::<Atlas>()
            .add_font(assets::fonts::GEIST_MONO)
            .unwrap();
        app.insert_resource(FontBook::new(font));
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
        // Exercise dragging on the populated page, with real Home input routing.
        let pages = app.widget::<crate::Home>(home).unwrap().pages.clone();
        app.component_mut::<LayoutStyle>(pages[0]).unwrap().display = Display::Hidden;
        let page = pages[3].id();
        app.component_mut::<LayoutStyle>(page).unwrap().display = Display::Flex;
        app.widget_mut::<crate::Home>(home).unwrap().active_page = 3;
        app.spawn(root, TestClock(page));
        // One tick lays the wrappers out; only then do the items' layout
        // handlers write their content insets, which a second tick applies.
        app.tick();
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
            .map(|&item| self.app.widget::<HomeItem>(item).unwrap().placement)
            .collect()
    }

    fn rect(&self, index: usize) -> Rect {
        self.app
            .component::<ComputedLayout>(self.page().items[index])
            .unwrap()
            .rect
    }

    /// Window-local centre of a node's rect, converted to the page-local
    /// coordinates `input` expects: `input` adds the (40, 60) page origin
    /// back on when signalling.
    fn center_of(&self, node: impl Into<NodeId>) -> (f32, f32) {
        let rect = self.app.component::<ComputedLayout>(node).unwrap().rect;
        (
            rect.x() + rect.width() / 2.0 - 40.0,
            rect.y() + rect.height() / 2.0 - 60.0,
        )
    }

    /// Page-local centre of an item's draggable content square: a press
    /// here starts a drag on that item.
    fn content_center(&self, index: usize) -> (f32, f32) {
        let content = self
            .app
            .widget::<HomeItem>(self.page().items[index])
            .unwrap()
            .content;
        self.center_of(content)
    }

    /// Page-local centre of an item's × button; laid out only in Edit mode.
    fn x_button_center(&self, index: usize) -> (f32, f32) {
        let button = self
            .app
            .widget::<HomeItem>(self.page().items[index])
            .unwrap()
            .x_button;
        self.center_of(button)
    }

    /// Synthesize a click on the debug Resize-mode toggle.
    fn click_debug_button(&mut self) {
        let button = self.page().debug_button.expect("the debug page has one");
        self.app.emit(
            Clicked {
                contact: ContactId::Touch(0),
                position: Point::new(56.0, 844.0),
            },
            button,
        );
        self.app.tick();
    }

    /// Page-local centre of an item's handle bead; laid out only in Resize
    /// mode. The array order matches `HandleEdge`'s declaration order.
    fn bead_center(&self, index: usize, edge: HandleEdge) -> (f32, f32) {
        let item = self
            .app
            .widget::<HomeItem>(self.page().items[index])
            .unwrap();
        self.center_of(item.resize_handles[edge as usize])
    }

    /// Spawn a debug icon at `placement`, register it with the page, and
    /// settle its layout. Returns its index in `items`.
    fn add_icon(&mut self, placement: Placement) -> usize {
        let number = self.page().items.len() + 1;
        let item = self
            .app
            .spawn(self.page, HomeItemBuilder { placement, number });
        *self.app.component_mut::<LayoutStyle>(item).unwrap() = item_style(&placement);
        self.app
            .widget_mut::<HomePage>(self.page)
            .unwrap()
            .items
            .push(item);
        let index = self.page().items.len() - 1;
        let original = self.placements();
        self.app
            .widget_mut::<HomePage>(self.page)
            .unwrap()
            .occupancy = grid::occupancy(&original).unwrap();
        self.app.tick();
        index
    }

    fn assert_badges(&self, display: Display) {
        for &item in &self.page().items {
            let badge = self.app.widget::<HomeItem>(item).unwrap().x_button;
            assert_eq!(
                self.app.component::<LayoutStyle>(badge).unwrap().display,
                display
            );
        }
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
            assert_eq!(
                rect.x(),
                40.0 + placement.column as f32 * (480.0 / GRID_COLUMNS as f32)
            );
            assert_eq!(
                rect.y(),
                60.0 + placement.row as f32 * (800.0 / GRID_ROWS as f32)
            );
        }
    }
}

#[test]
fn long_press_starts_drag_without_lifting_and_live_preview_commits_on_release() {
    let mut ui = Ui::new();
    let original = ui.placements();
    let occupancy = ui.page().occupancy;
    let item = ui.page().items[0];
    ui.assert_badges(Display::Hidden);
    let press = ui.content_center(0);
    ui.press(press.0, press.1);
    // Exactly five pixels of slop: the press still counts as a long press.
    ui.input(0, ContactPhase::Moved, press.0 + 3.0, press.1 + 4.0);
    ui.timeout();
    assert_eq!(ui.page().mode, PageMode::Edit);
    ui.assert_badges(Display::Flex);
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

    // Drag 240 right and 200 down from the grab.
    ui.input(0, ContactPhase::Moved, press.0 + 240.0, press.1 + 200.0);
    let preview = ui.page().drag.as_ref().unwrap().preview.clone().unwrap();
    assert_eq!((preview[0].column, preview[0].row), (2, 2));
    assert_eq!((preview[1].column, preview[1].row), (3, 4));
    assert_eq!((ui.rect(0).x(), ui.rect(0).y()), (280.0, 260.0));
    // The neighbour has visibly moved BEFORE release.
    assert_eq!((ui.rect(1).x(), ui.rect(1).y()), (400.0, 460.0));
    assert_eq!(ui.placements(), original);
    assert_eq!(ui.page().occupancy, occupancy);

    ui.input(0, ContactPhase::Released, press.0 + 240.0, press.1 + 200.0);
    assert!(ui.page().drag.is_none());
    assert_eq!(ui.page().mode, PageMode::Edit);
    ui.assert_grid_layout(&preview);
    assert_eq!(
        ui.app.widget::<crate::Home>(ui.home).unwrap().active_page,
        3
    );
}

#[test]
fn edit_mode_grabs_immediately_and_cancel_restores_neighbours_and_dragged_item() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    let original = ui.placements();
    let press = ui.content_center(0);
    ui.press(press.0, press.1);
    assert!(ui.page().drag.is_some());
    assert!(
        !ui.app
            .resource::<Timers>()
            .is_running(ui.page().long_press_timer)
    );
    ui.input(0, ContactPhase::Moved, press.0 + 240.0, press.1 + 200.0);
    assert_ne!(ui.rect(1).y(), 360.0);

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
    ui.input(0, ContactPhase::Cancelled, press.0 + 240.0, press.1 + 200.0);
    assert!(ui.page().drag.is_none());
    assert_eq!(ui.page().mode, PageMode::Edit);
    ui.assert_grid_layout(&original);
}

#[test]
fn movement_within_one_candidate_still_follows_finger_and_return_restores_preview() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    let original = ui.placements();
    let press = ui.content_center(0);
    ui.press(press.0, press.1);
    // Two finger positions inside the same candidate cell.
    ui.input(0, ContactPhase::Moved, press.0 + 190.0, press.1 + 160.0);
    let preview = ui.page().drag.as_ref().unwrap().preview.clone();
    ui.input(0, ContactPhase::Moved, press.0 + 200.0, press.1 + 170.0);
    assert_eq!(ui.page().drag.as_ref().unwrap().preview, preview);
    assert_eq!((ui.rect(0).x(), ui.rect(0).y()), (240.0, 230.0));
    // Returning to the grab point restores the original preview.
    ui.input(0, ContactPhase::Moved, press.0, press.1);
    assert_eq!(
        ui.page().drag.as_ref().unwrap().preview.as_ref().unwrap(),
        &original
    );
    assert_eq!(ui.rect(1).y(), 360.0);
    ui.input(0, ContactPhase::Released, press.0, press.1);
    ui.assert_grid_layout(&original);
}

#[test]
fn release_uses_its_final_position_and_clamps_whole_footprint() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    let press = ui.content_center(0);
    ui.press(press.0, press.1);
    ui.input(0, ContactPhase::Released, 2000.0, 2000.0);
    let placements = ui.placements();
    assert_eq!(
        (placements[0].row, placements[0].column),
        (GRID_ROWS - 2, GRID_COLUMNS - 2)
    );
    ui.assert_grid_layout(&placements);
}

#[test]
fn invalid_drop_rolls_back_a_packed_grid() {
    let mut ui = Ui::new();
    ui.app.widget_mut::<HomePage>(ui.page).unwrap().mode = PageMode::Edit;
    // Fill every remaining cell with icons. Moving the row-3 icon into
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
            let number = ui.page().items.len() + 1;
            let item = ui.app.spawn(ui.page, HomeItemBuilder { placement, number });
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
    let press = ui.content_center(1);
    ui.press(press.0, press.1);
    // One cell left: a valid swap with the icon there.
    ui.input(0, ContactPhase::Moved, press.0 - 120.0, press.1);
    assert!(ui.page().drag.as_ref().unwrap().preview.is_some());
    // Into the 2x2 widget's corner: no contiguous 2x2 home exists for it.
    ui.input(0, ContactPhase::Moved, press.0 - 360.0, press.1 - 300.0);
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
    ui.input(0, ContactPhase::Released, press.0 - 360.0, press.1 - 300.0);
    ui.assert_grid_layout(&original);
}

#[test]
fn empty_space_long_press_toggles_mode_without_dragging() {
    let mut ui = Ui::new();
    ui.assert_badges(Display::Hidden);
    for expected in [PageMode::Edit, PageMode::Normal] {
        ui.press(300.0, 100.0);
        ui.timeout();
        assert_eq!(ui.page().mode, expected);
        ui.assert_badges(if expected == PageMode::Edit {
            Display::Flex
        } else {
            Display::Hidden
        });
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
    let press = ui.content_center(0);
    ui.press(press.0, press.1);
    ui.input(0, ContactPhase::Released, press.0, press.1);
    ui.press(press.0, press.1);
    let id = ui.page().long_press_timer;
    ui.app.emit(Timeout { id }, ui.page);
    ui.app.tick();
    assert_eq!(ui.page().mode, PageMode::Normal);
    assert!(!ui.page().long_press.as_ref().unwrap().claimed);
    ui.timeout();
    assert_eq!(ui.page().mode, PageMode::Edit);
    assert!(ui.page().drag.is_some());
}

#[test]
fn debug_button_toggles_resize_mode_and_hides_x_buttons() {
    let mut ui = Ui::new();
    ui.assert_badges(Display::Hidden);

    // Enter Resize via the debug button: the × stays hidden.
    ui.click_debug_button();
    assert_eq!(ui.page().mode, PageMode::Resize);
    ui.assert_badges(Display::Hidden);

    // The placeholder exit: a long press returns the page to Normal.
    ui.press(300.0, 100.0);
    ui.timeout();
    assert_eq!(ui.page().mode, PageMode::Normal);

    // The toggle's other direction: click enters, click leaves.
    ui.click_debug_button();
    assert_eq!(ui.page().mode, PageMode::Resize);
    ui.click_debug_button();
    assert_eq!(ui.page().mode, PageMode::Normal);
}

#[test]
fn x_button_click_removes_the_item_and_rebuilds_occupancy() {
    let mut ui = Ui::new();
    // Enter Edit mode so the × buttons are shown and laid out.
    ui.press(300.0, 100.0);
    ui.timeout();
    assert_eq!(ui.page().mode, PageMode::Edit);
    ui.input(0, ContactPhase::Released, 300.0, 100.0);

    let removed = ui.page().items[0];
    // The survivors keep their placements; their occupancy indices shift down.
    let survivors: Vec<Placement> = ui.placements().iter().skip(1).cloned().collect();

    // The × sits in the wrapper's margin, so its press claims the button
    // alone: no drag starts, no mode toggle, no page swipe.
    let (x, y) = ui.x_button_center(0);
    ui.press(x, y);
    ui.input(0, ContactPhase::Released, x, y);

    assert!(!ui.app.is_live(removed));
    let page = ui.page();
    assert_eq!(page.items.len(), 2);
    assert_eq!(ui.placements(), survivors);
    assert_eq!(page.occupancy, grid::occupancy(&survivors).unwrap());
    assert_eq!(page.mode, PageMode::Edit);
    assert!(page.drag.is_none());
}

#[test]
fn removal_request_during_a_drag_is_refused() {
    let mut ui = Ui::new();
    // Enter Edit mode so the × buttons are shown and laid out.
    ui.press(300.0, 100.0);
    ui.timeout();
    ui.input(0, ContactPhase::Released, 300.0, 100.0);

    // Finger 0 drags item 0...
    let press = ui.content_center(0);
    ui.press(press.0, press.1);
    assert!(ui.page().drag.is_some());

    // ...while finger 1 clicks item 0's ×.
    let (x, y) = ui.x_button_center(0);
    ui.input(1, ContactPhase::Moved, x, y);
    ui.input(1, ContactPhase::Pressed, x, y);
    ui.input(1, ContactPhase::Released, x, y);

    // The request was refused: the drag and all three items survive.
    assert!(ui.page().drag.is_some());
    assert_eq!(ui.page().items.len(), 3);
    let original = ui.placements();
    assert_eq!(ui.page().occupancy, grid::occupancy(&original).unwrap());

    // Ending the drag afterwards still commits cleanly.
    ui.input(0, ContactPhase::Released, press.0, press.1);
    assert!(ui.page().drag.is_none());
    ui.assert_grid_layout(&original);
}

#[test]
fn right_handle_grow_snaps_by_cell_and_commits_on_release() {
    let mut ui = Ui::new();
    ui.click_debug_button();
    assert_eq!(ui.page().mode, PageMode::Resize);

    let original = ui.placements();
    let press = ui.bead_center(0, HandleEdge::Right);
    ui.press(press.0, press.1);
    assert!(ui.page().drag.is_some());
    // The claim routed the press to the page: no mode flip, no long press.
    assert_eq!(ui.page().mode, PageMode::Resize);
    assert!(ui.page().long_press.is_none());

    // Drag the right edge one full cell: the boundary snaps to width 3.
    ui.input(0, ContactPhase::Moved, press.0 + 120.0, press.1);
    let preview = ui.page().drag.as_ref().unwrap().preview.clone().unwrap();
    assert_eq!(
        (preview[0].width, preview[0].column, preview[0].row),
        (3, 0, 0)
    );
    // The wrapper grew grid-snapped, live, before any release.
    assert_eq!((ui.rect(0).width(), ui.rect(0).height()), (360.0, 200.0));
    // Nothing is committed while the finger is down.
    assert_eq!(ui.placements(), original);

    // A wiggle inside the same boundary changes nothing.
    ui.input(0, ContactPhase::Moved, press.0 + 130.0, press.1);
    assert_eq!(
        ui.page().drag.as_ref().unwrap().preview.as_ref().unwrap(),
        &preview
    );

    ui.input(0, ContactPhase::Released, press.0 + 130.0, press.1);
    assert!(ui.page().drag.is_none());
    assert_eq!(ui.page().mode, PageMode::Resize);
    ui.assert_grid_layout(&preview);
}

#[test]
fn right_handle_shrink_fits_without_moving_anyone() {
    let mut ui = Ui::new();
    // An icon directly under item 0's left column: a shrink must leave it
    // exactly where it is.
    let icon = ui.add_icon(Placement {
        row: 2,
        column: 0,
        width: 1,
        height: 1,
    });
    ui.click_debug_button();

    let press = ui.bead_center(0, HandleEdge::Right);
    ui.press(press.0, press.1);
    // Drag the right edge one full cell inward: width 2 -> 1.
    ui.input(0, ContactPhase::Moved, press.0 - 120.0, press.1);
    let preview = ui.page().drag.as_ref().unwrap().preview.clone().unwrap();
    assert_eq!((preview[0].width, preview[0].column), (1, 0));
    // The freed cells displaced nobody: the icon keeps its placement and
    // its on-screen rect, while the wrapper shrinks live.
    assert_eq!(preview[icon], ui.placements()[icon]);
    assert_eq!((ui.rect(icon).x(), ui.rect(icon).y()), (40.0, 260.0));
    assert_eq!(ui.rect(0).width(), 120.0);

    ui.input(0, ContactPhase::Released, press.0 - 120.0, press.1);
    ui.assert_grid_layout(&preview);
}

#[test]
fn left_handle_grow_moves_the_anchor_and_displaces_live() {
    let mut ui = Ui::new();
    // An icon beside item 1: growing leftward covers it.
    let icon = ui.add_icon(Placement {
        row: 3,
        column: 2,
        width: 1,
        height: 1,
    });
    ui.click_debug_button();

    let press = ui.bead_center(1, HandleEdge::Left);
    ui.press(press.0, press.1);
    // Two cells left: the left edge snaps to column 1, the pinned right
    // edge turns the span into 3 columns.
    ui.input(0, ContactPhase::Moved, press.0 - 240.0, press.1);
    let preview = ui.page().drag.as_ref().unwrap().preview.clone().unwrap();
    assert_eq!(
        (preview[1].column, preview[1].width, preview[1].row),
        (1, 3, 3)
    );
    // The displaced icon relocated to its nearest free cell, visibly,
    // BEFORE any release.
    assert_eq!((preview[icon].row, preview[icon].column), (2, 2));
    assert_eq!((ui.rect(icon).x(), ui.rect(icon).y()), (280.0, 260.0));

    ui.input(0, ContactPhase::Released, press.0 - 240.0, press.1);
    assert!(ui.page().drag.is_none());
    ui.assert_grid_layout(&preview);
}

#[test]
fn impossible_grow_snaps_home_and_rolls_back() {
    let mut ui = Ui::new();
    // Fill every empty cell: whatever the grow covers has nowhere to go.
    for row in 0..GRID_ROWS {
        for column in 0..GRID_COLUMNS {
            if ui.page().occupancy[row][column].is_some() {
                continue;
            }
            ui.add_icon(Placement {
                row,
                column,
                width: 1,
                height: 1,
            });
        }
    }
    let original = ui.placements();
    ui.click_debug_button();

    let press = ui.bead_center(0, HandleEdge::Bottom);
    ui.press(press.0, press.1);
    // Grow downward: the icons under the new row cannot be relocated.
    ui.input(0, ContactPhase::Moved, press.0, press.1 + 120.0);
    assert!(ui.page().drag.as_ref().unwrap().preview.is_none());
    // The wrapper refused to grow: it snapped back to its committed rect.
    assert_eq!((ui.rect(0).x(), ui.rect(0).y()), (40.0, 60.0));

    ui.input(0, ContactPhase::Released, press.0, press.1 + 120.0);
    assert!(ui.page().drag.is_none());
    assert_eq!(ui.page().mode, PageMode::Resize);
    ui.assert_grid_layout(&original);
}

#[test]
fn holding_a_bead_never_triggers_the_mode_toggle() {
    let mut ui = Ui::new();
    ui.click_debug_button();
    let press = ui.bead_center(0, HandleEdge::Right);
    ui.press(press.0, press.1);

    // The claim's PressCancelled, in the same flush, already cancelled the
    // long press the page's on_press started: no timer runs.
    assert!(ui.page().drag.is_some());
    assert!(ui.page().long_press.is_none());
    assert!(
        !ui.app
            .resource::<Timers>()
            .is_running(ui.page().long_press_timer)
    );

    // A timeout landing now changes nothing: there is no pending press.
    ui.timeout();
    assert_eq!(ui.page().mode, PageMode::Resize);
    assert!(ui.page().drag.is_some());

    ui.input(0, ContactPhase::Released, press.0, press.1);
    assert!(ui.page().drag.is_none());
    assert_eq!(ui.page().mode, PageMode::Resize);
}
