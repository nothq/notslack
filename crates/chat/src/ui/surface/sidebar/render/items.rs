use super::super::rows::{
    slack_sidebar_item_badge, slack_sidebar_item_badge_count, slack_sidebar_item_body,
    slack_sidebar_item_row, slack_sidebar_item_unread, SlackSidebarItemBodySpec,
};
use super::{
    div, img, px, rgb, slack_icon, slack_palette, AnyElement, AppearanceMode, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SlackShellIcon, SlackSidebarRow, SlackSidebarRowKind,
    SlackSidebarSectionIndicator, Styled, SurfaceState, SLACK_SIDEBAR_ROW_HEIGHT,
};
use crate::ui::{svg_from_body, SlackConversationKind, SlackSidebarItem};
use gpui::{Image, Role, SharedString, StatefulInteractiveElement};
use std::sync::{Arc, OnceLock};

mod actions;
mod shortcuts;

struct SlackSidebarItemSpec<'a> {
    item: &'a SlackSidebarItem,
    label: &'a SharedString,
    secondary_context: Option<&'a SharedString>,
    finder_group_count_label: Option<&'a SharedString>,
}

struct SlackSidebarSectionHeaderSpec<'a> {
    label: &'a str,
    indicator: SlackSidebarSectionIndicator,
    collapsible: bool,
}

impl SurfaceState {
    pub(crate) fn render_slack_sidebar_row(
        &self,
        row: &SlackSidebarRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match &row.kind {
            SlackSidebarRowKind::Shortcut { label, badge } => match label.as_str() {
                "Threads" => self.render_slack_threads_shortcut(cx),
                "Huddles" => self.render_slack_huddles_shortcut(cx),
                "Drafts & sent" => {
                    self.render_slack_drafts_sent_shortcut(label, badge.as_deref(), cx)
                }
                "Directories" => self.render_slack_directories_shortcut(cx),
                "Create a section" => {
                    self.render_slack_create_section_shortcut(badge.as_deref(), cx)
                }
                _ => panic!("unsupported Slack sidebar shortcut {label}"),
            },
            SlackSidebarRowKind::SectionHeader {
                label,
                indicator,
                collapsible,
            } => self.render_slack_sidebar_section_header_kind(
                SlackSidebarSectionHeaderSpec {
                    label,
                    indicator: *indicator,
                    collapsible: *collapsible,
                },
                cx,
            ),
            SlackSidebarRowKind::Separator => {
                self.render_slack_sidebar_separator().into_any_element()
            }
            SlackSidebarRowKind::Spacer { height } => {
                div().w_full().h(px(*height)).into_any_element()
            }
            SlackSidebarRowKind::DropHint => self.render_slack_drop_hint().into_any_element(),
            SlackSidebarRowKind::Item {
                item,
                label,
                secondary_context,
                finder_group_count_label,
                ..
            } => self.render_slack_sidebar_item_kind(
                SlackSidebarItemSpec {
                    item,
                    label,
                    secondary_context: secondary_context.as_ref(),
                    finder_group_count_label: finder_group_count_label.as_ref(),
                },
                cx,
            ),
        }
    }

    fn render_slack_sidebar_section_header_kind(
        &self,
        spec: SlackSidebarSectionHeaderSpec<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_section_header(spec.label, spec.indicator, spec.collapsible, cx)
    }

    fn render_slack_sidebar_item_kind(
        &self,
        spec: SlackSidebarItemSpec<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_sidebar_item(spec, cx)
    }

    fn render_slack_sidebar_item(
        &self,
        spec: SlackSidebarItemSpec<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_generic_item_row(spec, cx)
    }

    fn render_slack_generic_item_row(
        &self,
        spec: SlackSidebarItemSpec<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item = spec.item;
        let unread = slack_sidebar_item_unread(item);
        let peer_notifications_paused = item.active
            && matches!(
                item.target_kind,
                SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
            )
            && self
                .slack_workspace()
                .is_some_and(|workspace| workspace.peer_notifications_paused);
        let row = slack_sidebar_item_row(item, self.appearance_mode)
            .child(slack_sidebar_item_body(
                SlackSidebarItemBodySpec {
                    item,
                    active: item.active,
                    label: spec.label.clone(),
                    secondary_context: spec.secondary_context.cloned(),
                    group_count_label: spec.finder_group_count_label.cloned(),
                    unread,
                    peer_notifications_paused,
                    appearance_mode: self.appearance_mode,
                    remote_images: &self.slack_remote_images,
                },
                cx,
            ))
            .when_some(slack_sidebar_item_badge_count(item), |this, count| {
                this.child(slack_sidebar_item_badge(item, count, self.appearance_mode))
            });
        self.bind_slack_sidebar_item_click(row, item, cx)
    }

    pub(crate) fn render_slack_section_header(
        &self,
        label: &str,
        indicator: SlackSidebarSectionIndicator,
        collapsible: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let expanded = collapsible && !self.is_slack_section_collapsed(label);
        div()
            .id(format!("slack-sidebar-section-{label}"))
            .role(Role::TreeItem)
            .aria_label(label)
            .aria_expanded(expanded)
            .focusable()
            .tab_stop(true)
            .w_full()
            .h(px(SLACK_SIDEBAR_ROW_HEIGHT))
            .mt(px(10.0))
            .px(px(8.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(8.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let label = label.to_string();
                    move |this, _: &MouseDownEvent, _, cx| {
                        if collapsible {
                            this.toggle_slack_section(&label, cx);
                        }
                    }
                }),
            )
            .on_key_down(cx.listener({
                let label = label.to_string();
                move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if collapsible
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_section(&label, cx);
                    }
                }
            }))
            .text_size(px(15.0))
            .line_height(px(SLACK_SIDEBAR_ROW_HEIGHT))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(palette.sidebar_section_text))
            .child(self.render_slack_section_header_indicator(indicator, label, collapsible, cx))
            .child(label.to_string())
            .into_any_element()
    }

    pub(super) fn render_slack_section_header_indicator(
        &self,
        indicator: SlackSidebarSectionIndicator,
        label: &str,
        collapsible: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w(px(16.0))
            .flex()
            .justify_center()
            .child(match indicator {
                SlackSidebarSectionIndicator::Star => slack_icon(
                    SlackShellIcon::HeaderStar,
                    palette.sidebar_section_icon,
                    16.0,
                    cx,
                ),
                SlackSidebarSectionIndicator::ExternalConnections => slack_icon(
                    SlackShellIcon::ExternalConnections,
                    palette.sidebar_section_icon,
                    16.0,
                    cx,
                ),
                SlackSidebarSectionIndicator::Channels => {
                    slack_channel_section_icon(self.appearance_mode, palette.sidebar_section_icon)
                }
                SlackSidebarSectionIndicator::DirectMessages => {
                    slack_icon(SlackShellIcon::Dm, palette.sidebar_section_icon, 16.0, cx)
                }
                SlackSidebarSectionIndicator::Apps => {
                    slack_icon(SlackShellIcon::Apps, palette.sidebar_section_icon, 16.0, cx)
                }
                SlackSidebarSectionIndicator::Chevron => div()
                    .w(px(16.0))
                    .text_size(px(12.0))
                    .text_color(rgb(palette.sidebar_section_icon))
                    .child(slack_section_header_chevron(
                        collapsible,
                        self.is_slack_section_collapsed(label),
                    ))
                    .into_any_element(),
            })
    }
}

fn slack_channel_section_icon(appearance_mode: AppearanceMode, fill: u32) -> AnyElement {
    static DARK_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
    static LIGHT_IMAGE: OnceLock<Arc<Image>> = OnceLock::new();

    let image = match appearance_mode {
        AppearanceMode::Dark => DARK_IMAGE.get_or_init(|| slack_channel_section_image(fill)),
        AppearanceMode::Light => LIGHT_IMAGE.get_or_init(|| slack_channel_section_image(fill)),
    };
    img(image.clone()).size(px(16.0)).into_any_element()
}

fn slack_channel_section_image(fill: u32) -> Arc<Image> {
    svg_from_body(
        "0 0 20 20",
        format!(
            r##"<path fill="#{fill:06x}" d="M14.5 1.75a3.75 3.75 0 0 1 3.75 3.75v9a3.75 3.75 0 0 1-3.75 3.75h-9a3.75 3.75 0 0 1-3.75-3.75v-9A3.75 3.75 0 0 1 5.5 1.75zm-9 1.5A2.25 2.25 0 0 0 3.25 5.5v9a2.25 2.25 0 0 0 2.25 2.25h9a2.25 2.25 0 0 0 2.25-2.25v-9a2.25 2.25 0 0 0-2.25-2.25zm3.152 2.215a.751.751 0 0 1 1.478.256l-.268 1.556h1.365l.313-1.81a.75.75 0 0 1 1.478.256l-.269 1.554h1.658a.751.751 0 0 1 0 1.5h-1.915l-.475 2.753H13.8a.75.75 0 0 1 0 1.5h-2.042l-.26 1.503a.75.75 0 0 1-1.478-.255l.215-1.248H8.869l-.26 1.501a.75.75 0 0 1-1.478-.255l.215-1.246H5.593a.75.75 0 0 1 .001-1.5h2.012l.475-2.753H6.2a.75.75 0 0 1 0-1.5h2.14zm.476 6.065h1.366l.474-2.753H9.603z"/>"##
        ),
    )
}

fn slack_section_header_chevron(collapsible: bool, collapsed: bool) -> &'static str {
    match (collapsible, collapsed) {
        (true, true) => "▸",
        (true, false) => "▾",
        (false, _) => "",
    }
}
