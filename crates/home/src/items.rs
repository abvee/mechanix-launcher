use mecha_wayland::prelude::*;

// items are 0 indexed
pub struct Placement {
    // initial row and column
    pub row: u32,
    pub column: u32,

    pub height: u32, // number of rows
    pub width: u32,  // number of columns
}

// TODO: make this not a tuple struct because we will also be passing the
// HomeWidget builder here
pub(super) struct HomeItem(pub(super) Placement);

impl Build for HomeItem {
    type Widget = HomeItem;
}

impl Widget for HomeItem {
    type Builder = HomeItem;
    fn build(item: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        *s.component_mut::<LayoutStyle>(me).unwrap() = LayoutStyle::default().absolute();

        // placeholder red item. This will be Paint::None and the actual widget
        // will be eventually drawn here
        *s.component_mut::<Paint>(me).unwrap() = Paint::Quad(Quad::new(Color::rgb(1.0, 0.0, 0.0)));
        item
    }
}
