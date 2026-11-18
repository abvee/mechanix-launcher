//! Exercise the real input routing without a compositor. Sending ContactInput
//! (rather than directly invoking handlers) verifies capture and child clicks.

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

/// Attach handlers to the existing logo as a stand-in for an interactive icon.
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
            .insert_resource(Atlas::new());

        let root = app.root();
        let window = app
            .spawn(
                root,
                window().layout(LayoutStyle::default().size(px(480.0), px(800.0))),
            )
            .id();
        let home = HomeScreen.build(&mut app, window);
        let logo = app
            .descendants(home)
            .find(|&node| app.widget::<Icon>(node).is_some())
            .expect("Home builds its logo");
        app.spawn(root, ObserveChild(logo));
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

    /// Positions are relative to the logo's centre so presses also reach a child.
    fn input(&mut self, finger: u32, phase: ContactPhase, dx: f32, dy: f32) {
        self.app.signal(ContactInput {
            window: self.window,
            contact: ContactId::Touch(finger),
            phase,
            position: Point::new(self.start.x + dx, self.start.y + dy),
            time: 0,
        });
        self.app.flush();
    }

    fn press(&mut self, finger: u32) {
        self.input(finger, ContactPhase::Moved, 0.0, 0.0);
        self.input(finger, ContactPhase::Pressed, 0.0, 0.0);
    }

    fn drag(&self) -> Option<&Drag> {
        self.app.widget::<Home>(self.home).unwrap().drag.as_ref()
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
fn builds_the_logo() {
    let ui = Ui::new();
    let icons = ui
        .app
        .descendants(ui.home)
        .filter(|&node| ui.app.widget::<Icon>(node).is_some())
        .count();
    assert_eq!(icons, 1);
    assert!(ui.drag().is_none());
}

#[test]
fn small_movement_leaves_the_child_click_intact() {
    let logs = capture_logs(|| {
        let mut ui = Ui::new();
        ui.press(0);
        // The axis threshold is strict: reaching it is not enough to claim.
        ui.input(0, ContactPhase::Moved, AXIS_LOCK_THRESHOLD, 0.0);
        assert!(!ui.drag().unwrap().claimed);
        ui.input(0, ContactPhase::Released, AXIS_LOCK_THRESHOLD, 0.0);
        assert!(ui.drag().is_none());
        assert_eq!(
            ui.app.resource::<ChildEvents>().clicks,
            [ContactId::Touch(0)]
        );
        assert!(ui.app.resource::<ChildEvents>().cancellations.is_empty());
    });
    assert!(!logs.contains("swipe"), "{logs}");
}

#[test]
fn horizontal_drag_claims_once_cancels_child_and_reports_final_direction() {
    for sign in [-1.0, 1.0] {
        let logs = capture_logs(|| {
            let mut ui = Ui::new();
            ui.press(0);
            ui.input(0, ContactPhase::Moved, sign * 20.0, 0.0);
            assert!(ui.drag().unwrap().claimed);
            ui.input(0, ContactPhase::Moved, sign * 40.0, 0.0);
            // Reverse before releasing: direction comes from final displacement.
            ui.input(0, ContactPhase::Moved, -sign * 30.0, 0.0);
            ui.input(0, ContactPhase::Released, -sign * 30.0, 0.0);
            assert!(ui.drag().is_none());
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
        let direction = if sign < 0.0 {
            "Right swipe"
        } else {
            "Left swipe"
        };
        let opposite = if sign < 0.0 {
            "Left swipe"
        } else {
            "Right swipe"
        };
        assert_eq!(logs.matches(direction).count(), 1, "{logs}");
        assert!(!logs.contains(opposite), "{logs}");
    }
}

#[test]
fn vertical_lock_survives_sideways_motion_and_resets_on_the_next_press() {
    let logs = capture_logs(|| {
        let mut ui = Ui::new();
        ui.press(0);
        ui.input(0, ContactPhase::Moved, 2.0, 20.0);
        ui.input(0, ContactPhase::Moved, 40.0, 20.0);
        assert!(!ui.drag().unwrap().claimed);
        assert!(ui.app.resource::<ChildEvents>().cancellations.is_empty());
        ui.input(0, ContactPhase::Released, 40.0, 20.0);
        assert!(ui.drag().is_none());

        ui.press(1);
        ui.input(1, ContactPhase::Moved, -20.0, 0.0);
        assert!(
            ui.drag().unwrap().claimed,
            "a new press gets a fresh axis lock"
        );
        ui.input(1, ContactPhase::Released, -20.0, 0.0);
    });
    assert!(!logs.contains("Right swipe"), "{logs}");
    assert_eq!(logs.matches("Left swipe").count(), 1, "{logs}");
}

#[test]
fn secondary_contacts_cannot_move_or_clear_the_active_drag() {
    let mut ui = Ui::new();
    ui.press(0);
    ui.press(1);
    ui.input(1, ContactPhase::Moved, 40.0, 0.0);
    assert_eq!(ui.drag().unwrap().contact, ContactId::Touch(0));
    assert!(!ui.drag().unwrap().claimed);
    ui.input(1, ContactPhase::Released, 40.0, 0.0);
    assert_eq!(ui.drag().unwrap().contact, ContactId::Touch(0));

    ui.press(2);
    ui.input(2, ContactPhase::Cancelled, 0.0, 0.0);
    assert_eq!(ui.drag().unwrap().contact, ContactId::Touch(0));
    ui.input(0, ContactPhase::Released, 0.0, 0.0);
    assert!(ui.drag().is_none());
}

#[test]
fn cancelling_a_claimed_drag_allows_the_next_press() {
    let mut ui = Ui::new();
    ui.press(0);
    ui.input(0, ContactPhase::Moved, 20.0, 0.0);
    assert!(ui.drag().unwrap().claimed);
    ui.input(0, ContactPhase::Cancelled, 20.0, 0.0);
    assert!(ui.drag().is_none());
    assert!(ui.app.resource::<ChildEvents>().clicks.is_empty());

    ui.press(1);
    ui.input(1, ContactPhase::Released, 0.0, 0.0);
    assert_eq!(
        ui.app.resource::<ChildEvents>().clicks,
        [ContactId::Touch(1)]
    );
}

#[test]
fn child_claim_cancels_homes_pending_gesture() {
    let mut ui = Ui::new();
    ui.app.resource_mut::<ChildEvents>().claim_on_move = true;
    ui.press(0);
    // The child claims before Home's handler runs. Even a horizontal move
    // must not let Home reclaim the contact from that child.
    ui.input(0, ContactPhase::Moved, 20.0, 0.0);
    assert!(ui.drag().is_none());
    ui.input(0, ContactPhase::Released, 20.0, 0.0);
    assert_eq!(
        ui.app.resource::<ChildEvents>().clicks,
        [ContactId::Touch(0)]
    );
    assert!(ui.app.resource::<ChildEvents>().cancellations.is_empty());
}
