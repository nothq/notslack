use super::{
    div, px, rgb, slack_palette, AnyElement, AppearanceMode, Context, Div, InteractiveElement,
    IntoElement, ParentElement, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_TABS_HEIGHT,
};
use gpui::Role;

mod actions;
mod items;
mod layout;
mod overflow;

const SLACK_CONVERSATION_TAB_FONT_SIZE: f32 = 13.0;
const SLACK_CONVERSATION_TAB_LINE_HEIGHT: f32 = 18.0;
const SLACK_CONVERSATION_TAB_ICON_SIZE: f32 = 16.0;
const SLACK_CONVERSATION_TAB_GAP: f32 = 4.0;
const SLACK_CONVERSATION_TAB_HORIZONTAL_PADDING: f32 = 8.0;
const SLACK_CONVERSATION_TAB_MAX_WIDTH: f32 = 200.0;
const SLACK_CONVERSATION_TABS_LEFT_PADDING: f32 = 16.0;
const SLACK_CONVERSATION_TABS_RIGHT_PADDING: f32 = 12.0;
const SLACK_CONVERSATION_TABS_ADD_EDIT_RESERVE: f32 = 32.0;
const SLACK_CONVERSATION_TABS_MORE_ICON_SIZE: f32 = 13.0;
const SLACK_CONVERSATION_TABS_MORE_HORIZONTAL_PADDING: f32 = 6.0;
const SLACK_CONVERSATION_TABS_MORE_EXTRA_WIDTH: f32 = 29.0;
const SLACK_CONVERSATION_TABS_MENU_WIDTH: f32 = 200.0;
const SLACK_CONVERSATION_TABS_MENU_ROW_HEIGHT: f32 = 40.0;

impl SurfaceState {
    pub(crate) fn render_slack_main_tabs(
        &self,
        _workspace: &crate::ui::SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let surface = cx.entity().downgrade();
        div()
            .on_children_prepainted(move |bounds, window, cx| {
                let Some(bounds) = bounds.first().copied() else {
                    return;
                };
                let should_refresh = surface
                    .update(cx, |this, cx| {
                        this.update_slack_conversation_tabs_layout(bounds, window, cx)
                    })
                    .unwrap_or(false);
                if should_refresh {
                    window.refresh();
                }
            })
            .id("slack-conversation-tabs")
            .role(Role::TabList)
            .aria_label("Conversation views")
            .relative()
            .h(px(SLACK_TABS_HEIGHT))
            .flex_none()
            .child(self.render_slack_conversation_tabs_strip(cx))
            .into_any_element()
    }

    fn render_slack_conversation_tabs_strip(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let separator = match self.appearance_mode {
            AppearanceMode::Dark => 0x35373b,
            AppearanceMode::Light => palette.main_border,
        };
        let mut strip = div()
            .size_full()
            .relative()
            .pl(px(SLACK_CONVERSATION_TABS_LEFT_PADDING))
            .pr(px(SLACK_CONVERSATION_TABS_RIGHT_PADDING))
            .flex()
            .items_center()
            .gap(px(SLACK_CONVERSATION_TAB_GAP))
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .h(px(1.0))
                    .bg(rgb(separator)),
            );
        for spec in self
            .slack_conversation_tabs_cache
            .tabs
            .iter()
            .take(self.slack_conversation_tabs_cache.visible_count)
        {
            strip = strip.child(self.render_slack_conversation_tab(spec, cx));
        }
        if self.slack_conversation_tabs_cache.has_overflow() {
            strip = strip.child(self.render_slack_conversation_tabs_more(cx));
        }
        strip
    }
}
