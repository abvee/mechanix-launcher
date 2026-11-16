use mecha_wayland::prelude::*;

use super::{Borders, frame};

/// An empty, non-interactive tile that preserves its grid slot.
pub struct BlankTile;

pub struct BlankTileBuilder {
    pub borders: Borders,
}

impl Build for BlankTileBuilder {
    type Widget = BlankTile;
}

impl Widget for BlankTile {
    type Builder = BlankTileBuilder;

    fn build(b: Self::Builder, me: Handle<Self>, s: &mut Spawner<'_, Self>) -> Self {
        frame(me, s, b.borders);
        BlankTile
    }
}
