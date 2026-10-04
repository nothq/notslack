use super::{
    img, px, svg_from_body, svg_with_paths, AnyElement, Context, IconAsset, Image, IntoElement,
    Styled,
};
use gpui::{div, Animation, AnimationExt, ParentElement, RenderImage};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

mod action;
mod illustration;
mod media;
mod navigation;
mod reaction_picker;
mod section;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SlackShellIcon {
    Back,
    Forward,
    History,
    Clock,
    WorkspaceSwitcher,
    Slackbot,
    ArrowUp,
    ArrowDown,
    Home,
    Dm,
    Huddles,
    Activity,
    ActivityClear,
    Files,
    Folder,
    Later,
    Drafts,
    DraftsEdit,
    Canvas,
    Scheduled,
    EmptyDrafts,
    EmptyScheduled,
    More,
    MoreVertical,
    Admin,
    Directories,
    HashSmall,
    LockSmall,
    HeaderStar,
    ExternalConnections,
    Apps,
    ChannelFilled,
    Search,
    ListSearch,
    Compose,
    Plus,
    Upgrade,
    ThemeMoon,
    ThemeSun,
    MessageFilled,
    ReplyThread,
    MessageForward,
    MessageMenuEdit,
    MessageMenuMarkUnread,
    MessageMenuCopyLink,
    MessageMenuCopyMessage,
    Pins,
    People,
    PresenceActive,
    PresenceAway,
    PresenceDnd,
    PresenceDndFilled,
    NotificationsDndFilled,
    Bell,
    BellAlerting,
    BellOff,
    MenuCheck,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    DateDividerChevron,
    AttachmentCaretDown,
    AttachmentCaretRight,
    Attach,
    Emoji,
    FormatToggle,
    Mention,
    AddReaction,
    ReactionBarAdd,
    ReactionCategorySmileys,
    ReactionCategoryAnimals,
    ReactionCategoryFood,
    ReactionCategoryTravel,
    ReactionCategoryActivities,
    ReactionCategoryObjects,
    ReactionCategorySymbols,
    ReactionCategoryFlags,
    ReactionCategoryCustom,
    FormatBold,
    FormatItalic,
    FormatUnderline,
    FormatStrikethrough,
    FormatLink,
    FormatOrderedList,
    FormatBulletedList,
    FormatBlockquote,
    FormatCode,
    FormatCodeBlock,
    Video,
    Mic,
    Play,
    Pause,
    Volume,
    VolumeMuted,
    Send,
    Close,
    WarningFilled,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SlackIconCacheKey {
    icon: SlackShellIcon,
    fill: u32,
}

type SlackIconAssetCache = Mutex<HashMap<SlackIconCacheKey, Arc<IconAsset>>>;
pub(crate) type SlackSpinnerFrameCache = RefCell<HashMap<u32, Arc<Vec<Arc<RenderImage>>>>>;

static SLACK_ICON_ASSET_CACHE: OnceLock<SlackIconAssetCache> = OnceLock::new();
const SLACK_SPINNER_FRAME_COUNT: usize = 24;
const SLACK_SPINNER_ROTATION_DURATION: Duration = Duration::from_millis(800);

pub(crate) fn slack_icon<T: 'static>(
    icon: SlackShellIcon,
    fill: u32,
    size_px: f32,
    cx: &mut Context<T>,
) -> AnyElement {
    img(slack_icon_asset(icon, fill).render(cx))
        .size(px(size_px))
        .into_any_element()
}

pub(crate) fn slack_spinner<T: 'static>(
    cache: &SlackSpinnerFrameCache,
    animation_id: String,
    color: u32,
    size_px: f32,
    cx: &mut Context<T>,
) -> AnyElement {
    let frames = slack_spinner_frames(cache, color, cx);
    div()
        .size(px(size_px))
        .with_animation(
            animation_id,
            Animation::new(SLACK_SPINNER_ROTATION_DURATION).repeat(),
            move |spinner, progress| {
                let frame = ((progress * SLACK_SPINNER_FRAME_COUNT as f32) as usize)
                    % SLACK_SPINNER_FRAME_COUNT;
                spinner.child(img(frames[frame].clone()).size_full())
            },
        )
        .into_any_element()
}

fn slack_spinner_frames<T: 'static>(
    cache: &SlackSpinnerFrameCache,
    color: u32,
    cx: &mut Context<T>,
) -> Arc<Vec<Arc<RenderImage>>> {
    let mut cache = cache.borrow_mut();
    cache
        .entry(color)
        .or_insert_with(|| {
            Arc::new(
                (0..SLACK_SPINNER_FRAME_COUNT)
                    .map(|frame| {
                        let degrees = frame * 360 / SLACK_SPINNER_FRAME_COUNT;
                        IconAsset::new(svg_from_body(
                            "0 0 20 20",
                            format!(
                                r##"<g transform="rotate({degrees} 10 10)" fill="none" stroke="#{color:06x}" stroke-linecap="round" stroke-width="2"><circle cx="10" cy="10" r="7" opacity=".25"/><path d="M10 3a7 7 0 0 1 7 7"/></g>"##
                            ),
                        ))
                        .render(cx)
                    })
                    .collect(),
            )
        })
        .clone()
}

pub(crate) fn slack_empty_state_illustration<T: 'static>(
    icon: SlackShellIcon,
    width_px: f32,
    height_px: f32,
    cx: &mut Context<T>,
) -> AnyElement {
    img(slack_icon_asset(icon, 0).render(cx))
        .w(px(width_px))
        .h(px(height_px))
        .into_any_element()
}

fn slack_icon_asset(icon: SlackShellIcon, fill: u32) -> Arc<IconAsset> {
    let mut cache = SLACK_ICON_ASSET_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("Slack icon asset cache mutex poisoned");
    cache
        .entry(SlackIconCacheKey { icon, fill })
        .or_insert_with(|| Arc::new(IconAsset::new(slack_icon_image(icon, fill))))
        .clone()
}

fn slack_icon_image(icon: SlackShellIcon, fill: u32) -> Arc<Image> {
    navigation::slack_navigation_icon_image(icon, fill)
        .or_else(|| section::slack_section_icon_image(icon, fill))
        .or_else(|| action::slack_action_icon_image(icon, fill))
        .or_else(|| media::slack_media_icon_image(icon, fill))
        .or_else(|| reaction_picker::slack_reaction_picker_icon_image(icon, fill))
        .or_else(|| illustration::slack_illustration_image(icon))
        .expect("slack icon should be defined")
}

fn slack_even_odd_icon(view_box: &str, path: &str, fill: u32) -> Arc<Image> {
    svg_from_body(
        view_box,
        format!(
            r##"<path fill="#{fill:06x}" fill-rule="evenodd" clip-rule="evenodd" d="{path}"/>"##
        ),
    )
}
