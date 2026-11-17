use super::*;

#[test]
fn builds_the_logo() {
    let mut app = App::new();
    app.add_module(LayoutModule)
        .add_module(PaintModule)
        .add_module(MechanixTheme::dark())
        .insert_resource(Atlas::new());

    let root = app.root();
    let page = HomeScreen.build(&mut app, root);
    app.tick();

    let icons = app
        .descendants(page)
        .filter(|n| app.widget::<Icon>(*n).is_some())
        .count();
    assert_eq!(icons, 1);
}
