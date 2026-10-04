use std::collections::HashMap;
use std::sync::Arc;

use super::{
    div, img, px, rgb, slack_base_icon_radius, slack_icon, slack_palette, AnyElement,
    AppearanceMode, Context, Div, FluentBuilder, FontWeight, Image, IntoElement, ParentElement,
    SlackPalette, SlackShellIcon, SlackSidebarRow, SlackSidebarRowKind,
    SlackSidebarSectionIndicator, Styled, SurfaceState, SLACK_SIDEBAR_ROW_HEIGHT,
};

pub(crate) mod build;
mod visibility;
mod visibility_item;

pub(crate) use build::{build_slack_sidebar_rows, build_slack_sidebar_snapshot_rows};
#[cfg(test)]
pub(crate) use build::{shows_slack_more_unreads_above_pill, shows_slack_more_unreads_below_pill};
pub(crate) use visibility::{
    slack_sidebar_item_badge_count, slack_sidebar_item_body, slack_sidebar_item_row,
    slack_sidebar_item_row_with_active, slack_sidebar_item_unread, SlackSidebarItemBodySpec,
};
pub(crate) use visibility_item::slack_sidebar_item_badge;
