//! Actual layout of the content square and editing badge, without a compositor.

use super::*;
use mecha_wayland::mechanix_widgets::prelude::FontBook;

struct Ui {
    app: App,
    item: Handle<HomeItem>,
}

impl Ui {
    fn new() -> Self {
        let mut app = App::new();
        app.add_module(LayoutModule)
            .add_module(PaintModule)
            .add_module(WindowModule)
            .add_module(MechanixTheme::dark())
            .insert_resource(Atlas::new());
        let font = app
            .resource_mut::<Atlas>()
            .add_font(assets::fonts::GEIST_MONO)
            .unwrap();
        app.insert_resource(FontBook::new(font));
        let window = app.spawn(
            app.root(),
            window().layout(LayoutStyle::default().size(px(600.0), px(900.0))),
        );
        let item = app.spawn(
            window,
            HomeItemBuilder {
                placement: Placement {
                    row: 0,
                    column: 0,
                    width: 1,
                    height: 1,
                },
                number: 1,
            },
        );
        let mut ui = Self { app, item };
        ui.resize(120.0, 200.0);
        ui
    }

    fn settle(&mut self) {
        // First pass lays out the wrapper and emits its layout-change event;
        // the second applies the content insets written by that handler.
        self.app.tick();
        self.app.tick();
    }

    fn resize(&mut self, width: f32, height: f32) {
        *self.app.component_mut::<LayoutStyle>(self.item).unwrap() = LayoutStyle::default()
            .absolute()
            .inset(Insets::new(px(60.0), auto(), auto(), px(40.0)))
            .size(px(width), px(height));
        self.settle();
    }

    fn assert_square(&self, side: f32) {
        let item = self.app.widget::<HomeItem>(self.item).unwrap();
        let outer = self
            .app
            .component::<ComputedLayout>(self.item)
            .unwrap()
            .rect;
        let inner = self
            .app
            .component::<ComputedLayout>(item.content)
            .unwrap()
            .rect;
        assert_eq!((inner.width(), inner.height()), (side, side));
        assert_eq!(inner.x() - outer.x(), (outer.width() - side) / 2.0);
        assert_eq!(inner.y() - outer.y(), (outer.height() - side) / 2.0);
        // Changing the root's outer geometry must preserve its label alignment.
        let style = self.app.component::<LayoutStyle>(item.content).unwrap();
        let centered = LayoutStyle::default().center();
        assert_eq!(style.justify, centered.justify);
        assert_eq!(style.align_items, centered.align_items);
    }
}

#[test]
fn square_content_is_centered_in_tall_wide_and_resized_footprints() {
    let mut ui = Ui::new();
    ui.assert_square(64.0);
    ui.resize(240.0, 100.0);
    ui.assert_square(44.0);
    ui.resize(300.0, 300.0);
    ui.assert_square(244.0);
    ui.resize(40.0, 40.0);
    ui.assert_square(0.0);
}

#[test]
fn hiding_and_showing_the_wrapper_restores_the_square() {
    let mut ui = Ui::new();
    ui.app
        .component_mut::<LayoutStyle>(ui.item)
        .unwrap()
        .display = Display::Hidden;
    ui.settle();
    let content = ui.app.widget::<HomeItem>(ui.item).unwrap().content;
    let rect = ui.app.component::<ComputedLayout>(content).unwrap().rect;
    assert_eq!((rect.width(), rect.height()), (0.0, 0.0));
    ui.app
        .component_mut::<LayoutStyle>(ui.item)
        .unwrap()
        .display = Display::Flex;
    ui.settle();
    ui.assert_square(64.0);
}

#[test]
fn badge_visibility_changes_without_moving_or_resizing_the_content() {
    let mut ui = Ui::new();
    let badge = ui.app.widget::<HomeItem>(ui.item).unwrap().edit_badge;
    assert_eq!(
        ui.app.component::<LayoutStyle>(badge).unwrap().display,
        Display::Hidden
    );
    for mode in [PageMode::Edit, PageMode::Normal] {
        ui.app.emit(HomePageModeChanged(mode), ui.item);
        ui.settle();
        ui.assert_square(64.0);
        let rect = ui.app.component::<ComputedLayout>(badge).unwrap().rect;
        if mode == PageMode::Edit {
            assert_eq!(
                (rect.width(), rect.height()),
                (EDIT_BADGE_SIZE, EDIT_BADGE_SIZE)
            );
            assert_eq!(rect.x(), 40.0 + 120.0 - EDIT_BADGE_SIZE);
            assert_eq!(rect.y(), 60.0);
            let content = ui.app.widget::<HomeItem>(ui.item).unwrap().content;
            let content_rect = ui.app.component::<ComputedLayout>(content).unwrap().rect;
            assert!(rect.y() + rect.height() <= content_rect.y());
        } else {
            assert_eq!((rect.width(), rect.height()), (0.0, 0.0));
        }
    }
}
