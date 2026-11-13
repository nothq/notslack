//! The Add emoji dialog, laid out like Slack's: a Custom emoji tab for
//! uploading an image, and an Emoji packs tab.

use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, point, px, rgb, AnyElement, BoxShadow, Context, Div, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Role, Stateful,
    StatefulInteractiveElement, Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::surface::state::SLACK_EMOJI_PACK_CARD_EMOJI;
use crate::ui::{
    alpha,
    surface::{
        slack_icon, slack_palette, SlackAddEmojiDialog, SlackAddEmojiTab, SlackEmojiPacksLoad,
        SlackPalette, SlackShellIcon, SurfaceState,
    },
    SlackEmojiPack,
};

const SLACK_ADD_EMOJI_DIALOG_WIDTH: f32 = 540.0;
const SLACK_ADD_EMOJI_DIALOG_HEIGHT: f32 = 600.0;
const SLACK_ADD_EMOJI_PADDING: f32 = 28.0;
const SLACK_ADD_EMOJI_SAVE_GREEN: u32 = 0x007a5a;
const SLACK_ADD_EMOJI_REMOVE_RED: u32 = 0xe01e5a;
const SLACK_ADD_EMOJI_ERROR: u32 = 0xe01e5a;
const SLACK_EMOJI_PACK_DETAIL_COLUMNS: usize = 5;
const SLACK_EMOJI_PACK_CARD_COLUMNS: usize = 4;

impl SurfaceState {
    pub(super) fn render_slack_add_emoji_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let backdrop = dismissible_backdrop(
            div().absolute().inset_0().bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_add_emoji_dialog(cx);
            }),
            cx,
        );
        div()
            .id("slack-add-emoji-layer")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(backdrop)
            .child(self.render_slack_add_emoji_dialog(cx))
            .into_any_element()
    }

    fn render_slack_add_emoji_dialog(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let dialog = self
            .slack_add_emoji_dialog
            .as_ref()
            .expect("the Add emoji layer requires its dialog state");
        let body = match (dialog.tab, dialog.open_pack()) {
            (SlackAddEmojiTab::Packs, Some(pack)) => {
                return self.render_slack_emoji_pack_details(dialog, pack, palette, cx);
            }
            (SlackAddEmojiTab::Custom, _) => self
                .render_slack_add_custom_emoji(dialog, palette, cx)
                .into_any_element(),
            (SlackAddEmojiTab::Packs, None) => self
                .render_slack_emoji_packs(dialog, palette, cx)
                .into_any_element(),
        };
        slack_add_emoji_surface(palette, cx)
            .child(self.render_slack_add_emoji_header("Add emoji", palette, cx))
            .child(self.render_slack_add_emoji_tabs(dialog.tab, palette, cx))
            .child(body)
    }

    fn render_slack_add_emoji_header(
        &self,
        title: &str,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_none()
            .pt(px(20.0))
            .pl(px(SLACK_ADD_EMOJI_PADDING))
            .pr(px(16.0))
            .h(px(64.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child(title.to_string()),
            )
            .child(slack_add_emoji_close_button(palette, cx))
    }

    fn render_slack_add_emoji_tabs(
        &self,
        selected: SlackAddEmojiTab,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_none()
            .px(px(SLACK_ADD_EMOJI_PADDING - 8.0))
            .border_b_1()
            .border_color(rgb(palette.reaction_picker_border))
            .flex()
            .children(
                [
                    (SlackAddEmojiTab::Custom, "Custom emoji"),
                    (SlackAddEmojiTab::Packs, "Emoji packs"),
                ]
                .into_iter()
                .map(|(tab, label)| {
                    let active = tab == selected;
                    div()
                        .id(label)
                        .role(Role::Tab)
                        .aria_label(label)
                        .aria_selected(active)
                        .relative()
                        .h(px(36.0))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(if active {
                            palette.main_text
                        } else {
                            palette.main_secondary_text
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_slack_add_emoji_tab(tab, cx);
                        }))
                        .child(label)
                        .when(active, |this| {
                            this.child(
                                div()
                                    .absolute()
                                    .left(px(8.0))
                                    .right(px(8.0))
                                    .bottom(px(0.0))
                                    .h(px(2.0))
                                    .bg(rgb(palette.main_text)),
                            )
                        })
                }),
            )
    }

    fn render_slack_add_custom_emoji(
        &self,
        dialog: &SlackAddEmojiDialog,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        let name_input = self.slack_add_emoji_name_input(cx);
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_grow(1.0)
                    .px(px(SLACK_ADD_EMOJI_PADDING))
                    .pt(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(20.0))
                    .text_color(rgb(palette.main_text))
                    .child(div().text_size(px(15.0)).line_height(px(22.0)).child(
                        "Your custom emoji will be available to everyone in your workspace. \
                         You’ll find it in the custom tab of the emoji picker.",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(slack_add_emoji_step(
                                "1. Upload an image",
                                "Square images under 128KB and with transparent backgrounds work \
                                 best. If your image is too large, Slack will try to resize it.",
                                palette,
                            ))
                            .child(self.render_slack_add_emoji_image_row(dialog, palette, cx)),
                    )
                    .child(div().h(px(1.0)).bg(rgb(palette.reaction_picker_border)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(slack_add_emoji_step(
                                "2. Give it a name",
                                "This is also what you’ll type to add this emoji to your messages.",
                                palette,
                            ))
                            .when_some(name_input, |this, input| this.child(input)),
                    )
                    .when_some(dialog.error.clone(), |this, error| {
                        this.child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(SLACK_ADD_EMOJI_ERROR))
                                .child(error),
                        )
                    }),
            )
            .child(
                slack_add_emoji_footer()
                    .child(slack_add_emoji_secondary_button(
                        "slack-add-emoji-cancel",
                        "Cancel",
                        palette,
                        cx.listener(|this, _, _, cx| this.close_slack_add_emoji_dialog(cx)),
                    ))
                    .child(slack_add_emoji_primary_button(
                        "slack-add-emoji-save",
                        if dialog.saving { "Saving…" } else { "Save" },
                        SLACK_ADD_EMOJI_SAVE_GREEN,
                        !dialog.saving && dialog.image.is_some() && !dialog.name.is_empty(),
                        cx.listener(|this, _, _, cx| this.save_slack_custom_emoji(cx)),
                    )),
            )
    }

    fn render_slack_add_emoji_image_row(
        &self,
        dialog: &SlackAddEmojiDialog,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        // The image on light and on dark, as it will look in either theme.
        let preview = |background: u32, icon: u32, cx: &mut Context<Self>| {
            div()
                .size(px(64.0))
                .flex_none()
                .rounded(px(4.0))
                .border_1()
                .border_color(rgb(palette.reaction_picker_border))
                .bg(rgb(background))
                .flex()
                .items_center()
                .justify_center()
                .child(match &dialog.image {
                    Some(image) => img(image.preview.clone()).size(px(48.0)).into_any_element(),
                    None => slack_icon(SlackShellIcon::Files, icon, 24.0, cx),
                })
        };
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(preview(0xffffff, 0x1d1c1d, cx))
            .child(preview(0x1a1d21, 0xd1d2d3, cx))
            .child(
                div()
                    .ml(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(palette.main_secondary_text))
                            .max_w(px(220.0))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(dialog.image.as_ref().map_or_else(
                                || "Select an image".to_string(),
                                |image| image.file_name.clone(),
                            )),
                    )
                    .child(slack_add_emoji_secondary_button(
                        "slack-add-emoji-upload",
                        "Upload Image",
                        palette,
                        cx.listener(|this, _, _, cx| this.choose_slack_add_emoji_image(cx)),
                    )),
            )
    }

    fn render_slack_emoji_packs(
        &self,
        dialog: &SlackAddEmojiDialog,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let content = match &dialog.packs {
            SlackEmojiPacksLoad::Loading => slack_add_emoji_notice("Loading emoji packs…", palette),
            SlackEmojiPacksLoad::Failed(error) => {
                slack_add_emoji_notice(&format!("Couldn’t load emoji packs. {error}"), palette)
            }
            SlackEmojiPacksLoad::Loaded(packs) if packs.is_empty() => {
                slack_add_emoji_notice("Slack has no emoji packs for this workspace.", palette)
            }
            SlackEmojiPacksLoad::Loaded(packs) => {
                let (added, available): (Vec<_>, Vec<_>) =
                    packs.iter().partition(|pack| pack.installed);
                div().flex().flex_col().gap(px(12.0)).children(
                    [("Added", added), ("Available", available)]
                        .into_iter()
                        .filter(|(_, packs)| !packs.is_empty())
                        .map(|(title, packs)| {
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(palette.main_secondary_text))
                                        .child(format!("{title}  {}", packs.len())),
                                )
                                .child(div().flex().flex_wrap().gap(px(12.0)).children(
                                    packs.into_iter().map(|pack| {
                                        self.render_slack_emoji_pack_card(pack, palette, cx)
                                    }),
                                ))
                        }),
                )
            }
        };
        div()
            .id("slack-emoji-packs-scroll")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_y_scroll()
            .px(px(SLACK_ADD_EMOJI_PADDING))
            .py(px(16.0))
            .child(content)
    }

    fn render_slack_emoji_pack_card(
        &self,
        pack: &SlackEmojiPack,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let pack_id = pack.id.clone();
        let emoji = pack
            .emoji
            .iter()
            .take(SLACK_EMOJI_PACK_CARD_EMOJI)
            .collect::<Vec<_>>();
        div()
            .id(gpui::ElementId::Name(
                format!("slack-emoji-pack-{}", pack.id).into(),
            ))
            .role(Role::Button)
            .aria_label(pack.name.clone())
            .w(px(228.0))
            .p(px(16.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.reaction_picker_border))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.reaction_picker_hover_bg)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_emoji_pack(pack_id.clone(), cx);
            }))
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(slack_emoji_pack_title(pack, palette))
                    .when(pack.installed, |this| {
                        this.child(
                            div()
                                .size(px(22.0))
                                .rounded_full()
                                .bg(rgb(SLACK_ADD_EMOJI_SAVE_GREEN))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(slack_icon(SlackShellIcon::MenuCheck, 0xffffff, 14.0, cx)),
                        )
                    }),
            )
            .child(div().flex().flex_col().gap(px(8.0)).children(
                emoji.chunks(SLACK_EMOJI_PACK_CARD_COLUMNS).map(|row| {
                    div().flex().gap(px(12.0)).children(
                        row.iter()
                            .map(|emoji| self.render_slack_pack_emoji(&emoji.url, 36.0)),
                    )
                }),
            ))
    }

    fn render_slack_emoji_pack_details(
        &self,
        dialog: &SlackAddEmojiDialog,
        pack: &SlackEmojiPack,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let header = self.render_slack_emoji_pack_header(pack, palette, cx);
        let grid = div()
            .id("slack-emoji-pack-scroll")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_y_scroll()
            .px(px(SLACK_ADD_EMOJI_PADDING))
            .py(px(12.0))
            .flex()
            .flex_col()
            .gap(px(20.0))
            .children(
                pack.emoji
                    .chunks(SLACK_EMOJI_PACK_DETAIL_COLUMNS)
                    .map(|row| {
                        div().flex().justify_between().children(
                            row.iter()
                                .map(|emoji| self.render_slack_pack_emoji(&emoji.url, 64.0))
                                .chain(
                                    (row.len()..SLACK_EMOJI_PACK_DETAIL_COLUMNS)
                                        .map(|_| div().size(px(64.0)).into_any_element()),
                                ),
                        )
                    }),
            );
        let (label, color) = match (pack.installed, dialog.pack_pending) {
            (false, false) => ("Add pack", SLACK_ADD_EMOJI_SAVE_GREEN),
            (false, true) => ("Adding…", SLACK_ADD_EMOJI_SAVE_GREEN),
            (true, false) => ("Remove pack", SLACK_ADD_EMOJI_REMOVE_RED),
            (true, true) => ("Removing…", SLACK_ADD_EMOJI_REMOVE_RED),
        };
        slack_add_emoji_surface(palette, cx)
            .child(header)
            .child(grid)
            .child(
                slack_add_emoji_footer()
                    .border_t_1()
                    .border_color(rgb(palette.reaction_picker_border))
                    .when_some(dialog.error.clone(), |this, error| {
                        this.child(
                            div()
                                .flex_grow(1.0)
                                .text_size(px(13.0))
                                .text_color(rgb(SLACK_ADD_EMOJI_ERROR))
                                .child(error),
                        )
                    })
                    .child(slack_add_emoji_primary_button(
                        "slack-emoji-pack-toggle",
                        label,
                        color,
                        !dialog.pack_pending,
                        cx.listener(|this, _, _, cx| this.toggle_slack_open_emoji_pack(cx)),
                    )),
            )
    }

    fn render_slack_emoji_pack_header(
        &self,
        pack: &SlackEmojiPack,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        let count = pack.emoji.len();
        div()
            .flex_none()
            .pt(px(20.0))
            .pl(px(16.0))
            .pr(px(16.0))
            .h(px(72.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .id("slack-emoji-pack-back")
                    .role(Role::Button)
                    .aria_label("Go back")
                    .size(px(36.0))
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(palette.reaction_picker_hover_bg)))
                    .on_click(cx.listener(|this, _, _, cx| this.close_slack_emoji_pack(cx)))
                    .child(slack_icon(
                        SlackShellIcon::ChevronLeft,
                        palette.main_secondary_text,
                        20.0,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.main_text))
                            .child(pack.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(palette.main_secondary_text))
                            .child(format!("by {}  |  {count} emoji", pack.author)),
                    ),
            )
            .child(slack_add_emoji_close_button(palette, cx))
    }

    fn render_slack_pack_emoji(&self, url: &str, size: f32) -> AnyElement {
        match self.slack_remote_images.get(url) {
            Some(image) => img(image.clone())
                .size(px(size))
                .flex_none()
                .into_any_element(),
            None => div().size(px(size)).flex_none().into_any_element(),
        }
    }
}

fn slack_add_emoji_surface(palette: SlackPalette, cx: &mut Context<SurfaceState>) -> Stateful<Div> {
    div()
        .id("slack-add-emoji-dialog")
        .role(Role::Dialog)
        .aria_label("Add emoji")
        .relative()
        .w(px(SLACK_ADD_EMOJI_DIALOG_WIDTH))
        .h(px(SLACK_ADD_EMOJI_DIALOG_HEIGHT))
        .max_w(gpui::relative(0.92))
        .max_h(gpui::relative(0.92))
        .rounded(px(8.0))
        .bg(rgb(palette.main_bg))
        .shadow(slack_add_emoji_shadow())
        .overflow_hidden()
        .occlude()
        .flex()
        .flex_col()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|_: &mut SurfaceState, _: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
            }),
        )
}

fn slack_add_emoji_close_button(
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> Stateful<Div> {
    div()
        .id("slack-add-emoji-close")
        .role(Role::Button)
        .aria_label("Close")
        .size(px(36.0))
        .rounded(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.reaction_picker_hover_bg)))
        .on_click(cx.listener(|this, _, _, cx| this.close_slack_add_emoji_dialog(cx)))
        .child(slack_icon(
            SlackShellIcon::Close,
            palette.main_secondary_text,
            22.0,
            cx,
        ))
}

fn slack_add_emoji_step(title: &str, detail: &str, palette: SlackPalette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.main_text))
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(14.0))
                .line_height(px(20.0))
                .text_color(rgb(palette.main_secondary_text))
                .child(detail.to_string()),
        )
}

fn slack_emoji_pack_title(pack: &SlackEmojiPack, palette: SlackPalette) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(16.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.main_text))
                .child(pack.name.clone()),
        )
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(palette.main_secondary_text))
                .child(format!("by {}", pack.author)),
        )
}

fn slack_add_emoji_notice(text: &str, palette: SlackPalette) -> Div {
    div()
        .text_size(px(14.0))
        .text_color(rgb(palette.main_secondary_text))
        .child(text.to_string())
}

fn slack_add_emoji_footer() -> Div {
    div()
        .flex_none()
        .h(px(76.0))
        .px(px(SLACK_ADD_EMOJI_PADDING))
        .flex()
        .items_center()
        .justify_end()
        .gap(px(12.0))
}

fn slack_add_emoji_secondary_button(
    id: &'static str,
    label: &'static str,
    palette: SlackPalette,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .h(px(36.0))
        .px(px(16.0))
        .flex_none()
        .rounded(px(6.0))
        .border_1()
        .border_color(rgb(palette.main_secondary_text))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.reaction_picker_hover_bg)))
        .text_size(px(15.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(palette.main_text))
        .on_click(on_click)
        .child(label)
}

fn slack_add_emoji_primary_button(
    id: &'static str,
    label: &'static str,
    color: u32,
    enabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Stateful<Div> {
    let button = div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .h(px(36.0))
        .px(px(16.0))
        .flex_none()
        .rounded(px(6.0))
        .bg(rgb(color))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(15.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child(label);
    if enabled {
        button.cursor_pointer().on_click(on_click)
    } else {
        button.opacity(0.5)
    }
}

fn slack_add_emoji_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: alpha(0xe8e8e8, 0.13),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: false,
        },
        BoxShadow {
            color: alpha(0x000000, 0.35),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(48.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}
