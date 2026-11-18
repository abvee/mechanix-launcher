//! Run the home screen on its own, without the launcher shell.
//! `cargo run -p home --example home`
//!
//! In a Wayland session, press and drag horizontally with a mouse or touch
//! to see gesture logs. Home currently shows the logo; swipes do not change
//! pages yet.

use home::HomeScreen;
use mecha_wayland::mechanix_widgets::prelude::FontBook;
use mecha_wayland::prelude::*;
use screen::Screen;

/// Stop the standalone event loop when the compositor closes its window.
struct CloseHandler;

struct CloseHandlerBuilder {
    window: NodeId,
}

impl Build for CloseHandlerBuilder {
    type Widget = CloseHandler;
}

impl Widget for CloseHandler {
    type Builder = CloseHandlerBuilder;

    fn build(b: Self::Builder, _: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        s.on::<CloseRequested>(b.window, |ctx, _| ctx.signal(Stop));
        CloseHandler
    }
}

fn main() {
    // Examples have their own entry point; the launcher's subscriber is not run.
    tracing_subscriber::fmt().init();
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(WindowModule)
        .add_module(InteractivityModule)
        .add_module(AnimationModule)
        .add_module(RenderModule::default())
        .insert_resource(Atlas::new());
    app.add_module(MechanixTheme::dark());

    let font = app
        .resource_mut::<Atlas>()
        .add_font(assets::fonts::GEIST_MONO)
        .expect("Geist Mono font loads");
    app.insert_resource(FontBook::new(font));

    app.add_module(RingModule::default())
        .add_module(TimerModule)
        .add_module(
            WaylandModule::new()
                .bind::<WlCompositor>()
                .bind::<ZwpLinuxDmabufV1>()
                .bind::<XdgWmBase>()
                .bind::<WlSeat>(),
        )
        .add_module(PresentationModule {
            app_id: "mecha.home_example".into(),
            budget: Budget::default(),
        });

    let root = app.root();
    let window = app.spawn(
        root,
        window()
            .title("Home")
            .layout(LayoutStyle::default().size(px(480.0), px(800.0))),
    );
    app.spawn(
        root,
        CloseHandlerBuilder {
            window: window.id(),
        },
    );
    HomeScreen.build(&mut app, window.id());
    app.run();
}
