use std::ops::Range;

use gpui::{Pixels, Point};

use crate::ui::surface::SlackComposerLinkUrl;

/// The right-click menu of a link in the main composer.
#[derive(Clone, Debug)]
pub(crate) struct SlackComposerLinkMenu {
    pub(crate) range: Range<usize>,
    pub(crate) url: SlackComposerLinkUrl,
    pub(crate) position: Point<Pixels>,
}
