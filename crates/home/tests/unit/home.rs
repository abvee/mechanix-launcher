//! Exercise paging and real input routing without a compositor. ContactInput
//! verifies capture and child clicks; ticks apply layout before the next gesture.
//! Included as a unit-test module so private gesture state stays private.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use super::*;

#[derive(Default)]
struct ChildEvents {
    clicks: Vec<ContactId>,
    cancellations: Vec<ContactId>,
    claim_on_move: bool,
}
impl Resource for ChildEvents {}

/// Attach handlers to a page's logo as a stand-in for an interactive icon.
struct ObserveChild(NodeId);

impl Build for ObserveChild {
    type Widget = ObserveChild;
}

impl Widget for ObserveChild {
    type Builder = ObserveChild;

    fn build(b: Self, _: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        s.on::<Clicked>(b.0, |ctx, e| {
            ctx.resource_mut::<ChildEvents>().clicks.push(e.contact);
        })
        .on::<PressCancelled>(b.0, |ctx, e| {
            ctx.resource_mut::<ChildEvents>()
                .cancellations
                .push(e.contact);
        })
        .on::<PressMoved>(b.0, |ctx, e| {
            if ctx.resource::<ChildEvents>().claim_on_move {
                ctx.claim_contact(e.contact);
            }
        });
        b
    }
}

struct Ui {
    app: App,
    window: NodeId,
    home: NodeId,
    start: Point,
}

impl Ui {
    fn new() -> Self {
        let mut app = App::new();
        app.add_module(LayoutModule)
            .add_module(PaintModule)
            .add_module(WindowModule)
            .add_module(InteractivityModule)
            .add_module(MechanixTheme::dark())
            .init_resource::<ChildEvents>()
            .init_resource::<Timers>()
            .insert_resource(Atlas::new());

        let root = app.root();
        let window = app
            .spawn(
                root,
                window().layout(LayoutStyle::default().size(px(480.0), px(800.0))),
            )
            .id();
        let home = HomeScreen.build(&mut app, window);
        let first_page = app.widget::<Home>(home).unwrap().pages[0].id();
        let logo = app
            .descendants(first_page)
            .find(|&node| app.widget::<Icon>(node).is_some())
            .expect("the first page builds its logo");
        let logos: Vec<_> = app
            .descendants(home)
            .filter(|&node| app.widget::<Icon>(node).is_some())
            .collect();
        for logo in logos {
            app.spawn(root, ObserveChild(logo));
        }
        app.tick();

        let rect = app.component::<ComputedLayout>(logo).unwrap().rect;
        let start = Point::new(
            rect.x() + rect.width() / 2.0,
            rect.y() + rect.height() / 2.0,
        );
        Self {
            app,
            window,
            home,
            start,
        }
    }

    /// Every page centres its logo at the same point. Apply layout after input
    /// so subsequent presses use the newly displayed page's geometry.
    fn input(&mut self, finger: u32, phase: ContactPhase, dx: f32, dy: f32) {
        self.app.signal(ContactInput {
            window: self.window,
            contact: ContactId::Touch(finger),
            phase,
            position: Point::new(self.start.x + dx, self.start.y + dy),
            time: 0,
        });
        self.app.tick();
    }

    fn press(&mut self, finger: u32) {
        self.input(finger, ContactPhase::Moved, 0.0, 0.0);
        self.input(finger, ContactPhase::Pressed, 0.0, 0.0);
    }

    fn drag(&self) -> Option<&Drag> {
        self.app.widget::<Home>(self.home).unwrap().drag.as_ref()
    }

    fn swipe(&mut self, dx: f32) {
        self.press(0);
        self.input(0, ContactPhase::Moved, dx, 0.0);
        self.input(0, ContactPhase::Released, dx, 0.0);
    }

    /// Check both navigation state and actual page visibility/layout, rather
    /// than relying on the page-change log as evidence of a successful switch.
    fn assert_page(&self, expected: usize) {
        let home = self.app.widget::<Home>(self.home).unwrap();
        assert_eq!(home.active_page, expected);
        for (index, page) in home.pages.iter().enumerate() {
            let display = if index == expected {
                Display::Flex
            } else {
                Display::Hidden
            };
            assert_eq!(
                self.app
                    .component::<LayoutStyle>(page.id())
                    .unwrap()
                    .display,
                display,
                "page {index}"
            );
        }
        let visible = self
            .app
            .component::<ComputedLayout>(home.pages[expected].id())
            .unwrap()
            .rect;
        let window = self
            .app
            .component::<ComputedLayout>(self.window)
            .unwrap()
            .rect;
        assert_eq!(visible.width(), window.width());
        assert_eq!(visible.height(), window.height());
    }
}

/// Capture only this test thread's diagnostic events, without a global subscriber.
struct LogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for LogWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn capture_logs(f: impl FnOnce()) -> String {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = bytes.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_writer(move || LogWriter(writer.clone()))
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    String::from_utf8(bytes.lock().unwrap().clone()).unwrap()
}

#[test]
fn builds_four_pages_with_only_the_first_visible() {
    let ui = Ui::new();
    let home = ui.app.widget::<Home>(ui.home).unwrap();
    assert_eq!(home.pages.len(), 4);
    for page in &home.pages {
        let icons = ui
            .app
            .descendants(page.id())
            .filter(|&node| ui.app.widget::<Icon>(node).is_some())
            .count();
        assert_eq!(icons, 1);
    }
    ui.assert_page(0);
    assert!(ui.drag().is_none());
}

#[test]
fn taps_and_small_movements_leave_the_page_and_child_click_intact() {
    for dx in [0.0, AXIS_LOCK_THRESHOLD, -AXIS_LOCK_THRESHOLD] {
        let mut ui = Ui::new();
        ui.press(0);
        // The axis threshold is strict: reaching it is not enough to claim.
        ui.input(0, ContactPhase::Moved, dx, 0.0);
        assert!(!ui.drag().unwrap().claimed);
        ui.input(0, ContactPhase::Released, dx, 0.0);
        assert!(ui.drag().is_none());
        ui.assert_page(0);
        assert_eq!(
            ui.app.resource::<ChildEvents>().clicks,
            [ContactId::Touch(0)]
        );
        assert!(ui.app.resource::<ChildEvents>().cancellations.is_empty());
    }
}

#[test]
fn swipes_visit_each_adjacent_page_in_both_directions() {
    let mut ui = Ui::new();
    let pages = ui.app.widget::<Home>(ui.home).unwrap().pages.clone();
    for expected in 1..4 {
        // Just beyond axis lock is enough; there is no page-switch threshold.
        ui.swipe(-(AXIS_LOCK_THRESHOLD + 1.0));
        ui.assert_page(expected);
        assert!(ui.drag().is_none());
    }
    for expected in (0..3).rev() {
        // A longer swipe still advances only one page.
        ui.swipe(180.0);
        ui.assert_page(expected);
        assert!(ui.drag().is_none());
    }
    let home = ui.app.widget::<Home>(ui.home).unwrap();
    for (original, current) in pages.iter().zip(&home.pages) {
        assert_eq!(
            original.id(),
            current.id(),
            "switching preserves page nodes"
        );
    }
}

#[test]
fn outward_swipes_at_either_boundary_keep_the_current_page() {
    let mut ui = Ui::new();
    ui.swipe(30.0);
    ui.assert_page(0);
    assert!(ui.drag().is_none());

    for _ in 0..3 {
        ui.swipe(-30.0);
    }
    ui.assert_page(3);
    ui.swipe(-30.0);
    ui.assert_page(3);
    assert!(ui.drag().is_none());

    // A boundary swipe must not prevent a subsequent valid gesture.
    ui.swipe(30.0);
    ui.assert_page(2);
}

#[test]
fn horizontal_drag_claims_once_cancels_child_and_switches_on_final_direction() {
    for sign in [-1.0, 1.0] {
        let mut ui = Ui::new();
        // Start away from either boundary so both directions can switch.
        ui.swipe(-30.0);
        ui.assert_page(1);
        *ui.app.resource_mut::<ChildEvents>() = ChildEvents::default();
        let logs = capture_logs(|| {
            ui.press(0);
            ui.input(0, ContactPhase::Moved, sign * 20.0, 0.0);
            assert!(ui.drag().unwrap().claimed);
            ui.input(0, ContactPhase::Moved, sign * 40.0, 0.0);
            // Reverse before releasing: direction comes from final displacement.
            ui.input(0, ContactPhase::Moved, -sign * 30.0, 0.0);
            ui.assert_page(1); // Claiming/moving alone must not change pages.
            ui.input(0, ContactPhase::Released, -sign * 30.0, 0.0);
            assert!(ui.drag().is_none());
            ui.assert_page(if sign < 0.0 { 0 } else { 2 });
            assert_eq!(
                ui.app.resource::<ChildEvents>().cancellations,
                [ContactId::Touch(0)]
            );
            assert!(ui.app.resource::<ChildEvents>().clicks.is_empty());
        });
        assert_eq!(
            logs.matches("Claimed horizontal swipe").count(),
            1,
            "{logs}"
        );
    }
}

#[test]
fn returning_to_the_press_position_does_not_switch_pages() {
    let mut ui = Ui::new();
    ui.press(0);
    ui.input(0, ContactPhase::Moved, -30.0, 0.0);
    assert!(ui.drag().unwrap().claimed);
    ui.input(0, ContactPhase::Moved, 0.0, 0.0);
    ui.input(0, ContactPhase::Released, 0.0, 0.0);
    ui.assert_page(0);
    assert!(ui.drag().is_none());
    assert!(ui.app.resource::<ChildEvents>().clicks.is_empty());
}

#[test]
fn claimed_drag_needs_no_extra_distance_at_release() {
    let mut ui = Ui::new();
    ui.press(0);
    ui.input(0, ContactPhase::Moved, -30.0, 0.0);
    assert!(ui.drag().unwrap().claimed);
    ui.input(0, ContactPhase::Moved, -1.0, 0.0);
    ui.input(0, ContactPhase::Released, -1.0, 0.0);
    ui.assert_page(1);
}

#[test]
fn vertical_lock_prevents_paging_and_resets_on_the_next_press() {
    for dy in [-20.0, 20.0] {
        let mut ui = Ui::new();
        ui.press(0);
        ui.input(0, ContactPhase::Moved, -2.0, dy);
        ui.input(0, ContactPhase::Moved, -40.0, dy);
        assert!(!ui.drag().unwrap().claimed);
        assert!(ui.app.resource::<ChildEvents>().cancellations.is_empty());
        ui.input(0, ContactPhase::Released, -40.0, dy);
        assert!(ui.drag().is_none());
        ui.assert_page(0);

        ui.press(1);
        ui.input(1, ContactPhase::Moved, -20.0, 0.0);
        assert!(
            ui.drag().unwrap().claimed,
            "a new press gets a fresh axis lock"
        );
        ui.input(1, ContactPhase::Released, -20.0, 0.0);
        ui.assert_page(1);
    }
}

#[test]
fn secondary_contacts_cannot_move_or_clear_the_active_drag() {
    let mut ui = Ui::new();
    ui.press(0);
    ui.press(1);
    ui.input(1, ContactPhase::Moved, -40.0, 0.0);
    assert_eq!(ui.drag().unwrap().contact, ContactId::Touch(0));
    assert!(!ui.drag().unwrap().claimed);
    ui.input(1, ContactPhase::Released, -40.0, 0.0);
    assert_eq!(ui.drag().unwrap().contact, ContactId::Touch(0));
    ui.assert_page(0);

    ui.press(2);
    ui.input(2, ContactPhase::Cancelled, 0.0, 0.0);
    assert_eq!(ui.drag().unwrap().contact, ContactId::Touch(0));
    ui.input(0, ContactPhase::Released, 0.0, 0.0);
    assert!(ui.drag().is_none());
    ui.assert_page(0);
}

#[test]
fn cancelled_gestures_do_not_switch_and_allow_the_next_press() {
    for dx in [0.0, -30.0] {
        let mut ui = Ui::new();
        ui.press(0);
        ui.input(0, ContactPhase::Moved, dx, 0.0);
        assert_eq!(ui.drag().unwrap().claimed, dx != 0.0);
        ui.input(0, ContactPhase::Cancelled, dx, 0.0);
        assert!(ui.drag().is_none());
        assert!(ui.app.resource::<ChildEvents>().clicks.is_empty());
        ui.assert_page(0);

        ui.swipe(-30.0);
        ui.assert_page(1);
    }
}

#[test]
fn child_claim_cancels_homes_pending_gesture() {
    let mut ui = Ui::new();
    ui.app.resource_mut::<ChildEvents>().claim_on_move = true;
    ui.press(0);
    // The child claims before Home's handler runs. Even a horizontal move
    // must not let Home reclaim the contact from that child.
    ui.input(0, ContactPhase::Moved, -20.0, 0.0);
    assert!(ui.drag().is_none());
    ui.input(0, ContactPhase::Released, -20.0, 0.0);
    ui.assert_page(0);
    assert_eq!(
        ui.app.resource::<ChildEvents>().clicks,
        [ContactId::Touch(0)]
    );
    assert!(ui.app.resource::<ChildEvents>().cancellations.is_empty());
}
