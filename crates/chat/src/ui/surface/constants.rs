use super::{SlackRailItemSpec, SlackShellIcon};

pub(crate) const SLACK_HISTORY_HEIGHT: f32 = 40.0;
pub(crate) const SLACK_SIDEBAR_HEADER_HEIGHT: f32 = 49.0;
pub(crate) const SLACK_MAIN_HEADER_HEIGHT: f32 = 49.0;
pub(crate) const SLACK_TOP_NAV_LEFT_INSET: f32 = 6.0;
pub(crate) const SLACK_TOP_NAV_RIGHT_INSET: f32 = 8.0;
pub(crate) const SLACK_TOP_NAV_LEFT_FLEX_BASIS: f32 = 0.237_376;
pub(crate) const SLACK_TOP_SEARCH_MIN_WIDTH: f32 = 280.0;
pub(crate) const SLACK_TOP_SEARCH_MAX_WIDTH: f32 = 1000.0;
pub(crate) const SLACK_TOP_SEARCH_HEIGHT: f32 = 28.0;
pub(crate) const SLACK_SEARCH_RECENT_PLACE_LIMIT: usize = 5;
pub(crate) const SLACK_COMPOSER_INPUT_HEIGHT: f32 = 38.0;
pub(crate) const SLACK_COMPOSER_TOOLBAR_HEIGHT: f32 = 40.0;
pub(crate) const SLACK_MESSAGE_LIST_OVERDRAW: f32 = 128.0;
pub(crate) const SLACK_SIDEBAR_LIST_OVERDRAW: f32 = 84.0;
pub(crate) const SLACK_SIDEBAR_ROW_HEIGHT: f32 = 28.0;
pub(crate) const SLACK_COMPOSER_PICKER_POPOVER_WIDTH: f32 = 361.0;
pub(crate) const SLACK_COMPOSER_PICKER_LEFT_INSET: f32 = -4.0;
pub(crate) const SLACK_COMPOSER_PICKER_BOTTOM_OFFSET: f32 = 99.0;
pub(crate) const SLACK_EMOJI_PICKER_POPOVER_HEIGHT: f32 = 468.0;
pub(crate) const SLACK_MENTION_PICKER_POPOVER_HEIGHT: f32 = 224.0;
pub(crate) const SLACK_RAIL_ITEMS: [SlackRailItemSpec; 7] = [
    SlackRailItemSpec::new("Home", SlackShellIcon::Home),
    SlackRailItemSpec::new("DMs", SlackShellIcon::Dm),
    SlackRailItemSpec::new("Activity", SlackShellIcon::Activity),
    SlackRailItemSpec::new("Files", SlackShellIcon::Files),
    SlackRailItemSpec::new("Later", SlackShellIcon::Later),
    SlackRailItemSpec::new("More", SlackShellIcon::More),
    SlackRailItemSpec::new("Admin", SlackShellIcon::Admin),
];
