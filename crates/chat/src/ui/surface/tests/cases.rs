use base64::prelude::{Engine as _, BASE64_STANDARD};
use gpui::{
    size, AppContext, Context, HeadlessAppContext, IntoElement, KeyDownEvent, Keystroke, Pixels,
    Render, Size, Window,
};
use gpui_platform::{current_headless_renderer, current_platform};
use image::GenericImageView;
use remote_image_model::RemoteImageData;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard, OnceLock},
};

use crate::ui::{
    test_support::{
        slack_test_api, slack_test_api_with_conversations, slack_test_api_with_remote_images,
        slack_test_board, slack_test_board_with_workspace, slack_test_channels_section,
        slack_test_workspace, slack_test_workspace_with_message_count,
        slack_test_workspace_with_sections, SlackSendAttempt,
    },
    AppearanceMode, SlackAttachment, SlackComposerFormatAction, SlackPlaybackSpeed, SlackRailView,
    SlackUploadFile, SlackWorkspace, SlackWorkspaceApi, SurfaceInput, SurfaceRoot,
};

mod attachments;
mod composer;
mod images;
mod interactions;
mod media;
mod scroll_position;

fn headless_test_context() -> HeadlessAppContext {
    let platform = current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(platform.text_system(), Arc::new(()), || {
        current_headless_renderer()
    });
    cx.update(|cx| cx.set_global(AppearanceMode::Dark));
    cx
}

fn acquire_headless_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn app_size() -> Size<Pixels> {
    size(gpui::px(1200.0), gpui::px(800.0))
}

struct SlackTestApp {
    root: SurfaceRoot,
}

impl SlackTestApp {
    fn from_slack_workspace(
        workspace: SlackWorkspace,
        workspace_api: Arc<dyn SlackWorkspaceApi>,
    ) -> Self {
        let mut root = SurfaceRoot::from_input(SurfaceInput {
            workspace: Some(workspace),
            workspace_api: Some(workspace_api),
            local_file_api: None,
            embedded_shell: false,
            initial_thread_message_id: None,
        });
        root.set_preview_width(1280.0);
        root.set_viewport_height(720.0);
        Self { root }
    }
}

impl Render for SlackTestApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.root.render_standalone(cx)
    }
}

fn text_key_down_event(text: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke {
            modifiers: Default::default(),
            key: text.to_string(),
            key_char: Some(text.to_string()),
        },
        is_held: false,
        prefer_character_input: false,
    }
}

fn named_key_down_event(key: &str) -> KeyDownEvent {
    KeyDownEvent {
        keystroke: Keystroke::parse(key).expect("named keystroke should parse"),
        is_held: false,
        prefer_character_input: false,
    }
}
