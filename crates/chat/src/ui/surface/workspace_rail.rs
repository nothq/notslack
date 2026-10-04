mod attention;
mod host;
mod menu;
mod render;
mod shortcuts;

use gpui::{
    Context, FocusHandle, KeystrokeEvent, ScrollStrategy, SharedString, Subscription,
    UniformListScrollHandle, Window, WindowId,
};

use attention::slack_workspace_attention;
use shortcuts::slack_workspace_shortcut_index;

use crate::{
    model::{ChatTeamId, SlackWorkspaceDescriptor},
    ui::{initials, slack_avatar_fill, SurfaceState},
};

const WORKSPACE_ROW_HEIGHT: f32 = 52.0;
const WORKSPACE_RING_SIZE: f32 = 40.0;
const WORKSPACE_TILE_SIZE: f32 = 32.0;
const WORKSPACE_ADD_BUTTON_SIZE: f32 = 32.0;
const WORKSPACE_ADD_BUTTON_TOP_IN_ROW: f32 = 4.0;
const WORKSPACE_ADD_MENU_LEFT: f32 = 54.0;
const WORKSPACE_ADD_MENU_WIDTH: f32 = 300.0;
const WORKSPACE_ADD_MENU_HEIGHT: f32 = 108.0;
const WORKSPACE_ADD_MENU_ROW_HEIGHT: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SlackWorkspaceAttention {
    None,
    Unread,
    Mentions(u32),
}

struct SlackWorkspaceRailItem {
    team_id: ChatTeamId,
    workspace_name: SharedString,
    workspace_logo_url: Option<SharedString>,
    initials: SharedString,
    fallback_fill: u32,
    focus_handle: FocusHandle,
}

pub(super) struct SlackWorkspaceRail {
    items: Vec<SlackWorkspaceRailItem>,
    selected_team_id: ChatTeamId,
    attention: Vec<SlackWorkspaceAttention>,
    active: bool,
    expanded: bool,
    rendered_window_id: Option<WindowId>,
    pending_selection: Option<ChatTeamId>,
    scroll_handle: UniformListScrollHandle,
    surface_subscriptions: Vec<Option<Subscription>>,
    add_menu_open: bool,
    add_menu_selected_index: Option<usize>,
    add_button_focus_handle: FocusHandle,
    add_menu_focus_handle: FocusHandle,
    add_menu_focus_pending: bool,
    add_menu_restore_button_focus: bool,
    _shortcut_subscription: Subscription,
}

impl SlackWorkspaceRail {
    pub(super) fn new(
        descriptors: Vec<SlackWorkspaceDescriptor>,
        selected_team_id: ChatTeamId,
        cx: &mut Context<Self>,
    ) -> Self {
        let listener = cx.listener(|this, event: &KeystrokeEvent, window, cx| {
            let Some(index) = slack_workspace_shortcut_index(event) else {
                return;
            };
            if !this.active
                || this.rendered_window_id != Some(window.window_handle().window_id())
                || index >= this.items.len()
            {
                return;
            }
            this.request_selection(index, window, cx);
            cx.stop_propagation();
        });
        let shortcut_subscription = cx.intercept_keystrokes(listener);
        let items = descriptors
            .into_iter()
            .map(|descriptor| {
                let workspace_name = SharedString::from(descriptor.workspace_name);
                SlackWorkspaceRailItem {
                    team_id: descriptor.team_id,
                    workspace_logo_url: descriptor.workspace_logo_url.map(SharedString::from),
                    initials: initials(workspace_name.as_ref()).into(),
                    fallback_fill: slack_avatar_fill(workspace_name.as_ref()),
                    workspace_name,
                    focus_handle: cx.focus_handle(),
                }
            })
            .collect::<Vec<_>>();
        let attention = vec![SlackWorkspaceAttention::None; items.len()];
        let surface_subscriptions = std::iter::repeat_with(|| None).take(items.len()).collect();
        Self {
            items,
            selected_team_id,
            attention,
            active: false,
            expanded: false,
            rendered_window_id: None,
            pending_selection: None,
            scroll_handle: UniformListScrollHandle::new(),
            surface_subscriptions,
            add_menu_open: false,
            add_menu_selected_index: None,
            add_button_focus_handle: cx.focus_handle(),
            add_menu_focus_handle: cx.focus_handle(),
            add_menu_focus_pending: false,
            add_menu_restore_button_focus: false,
            _shortcut_subscription: shortcut_subscription,
        }
    }

    pub(super) fn sync_presentation(
        &mut self,
        selected_team_id: ChatTeamId,
        active: bool,
        expanded: bool,
        cx: &mut Context<Self>,
    ) {
        let selected_changed = self.selected_team_id != selected_team_id;
        let active_changed = self.active != active;
        let expanded_changed = self.expanded != expanded;
        let mut changed = selected_changed || active_changed || expanded_changed;
        self.selected_team_id = selected_team_id;
        self.active = active;
        self.expanded = expanded;
        if (!active || !expanded) && self.add_menu_open {
            self.add_menu_open = false;
            self.add_menu_selected_index = None;
            self.add_menu_focus_pending = false;
            self.add_menu_restore_button_focus = false;
            changed = true;
        }
        if selected_changed {
            let selected_index = self
                .items
                .iter()
                .position(|item| item.team_id == self.selected_team_id)
                .expect("selected Slack workspace must remain in the ordered rail");
            self.scroll_handle
                .scroll_to_item(selected_index, ScrollStrategy::Nearest);
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn attach_surface(
        &mut self,
        index: usize,
        surface: &gpui::Entity<SurfaceState>,
        cx: &mut Context<Self>,
    ) {
        let hydrated_team_id = surface
            .read(cx)
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone());
        if let Some(team_id) = hydrated_team_id {
            assert_eq!(
                team_id,
                self.items[index].team_id.as_str(),
                "Slack workspace rail surface must match its ordered team"
            );
        }
        if self.surface_subscriptions[index].is_some() {
            return;
        }
        self.refresh_surface_attention(index, surface, cx);
        self.surface_subscriptions[index] = Some(cx.observe(surface, move |this, surface, cx| {
            this.refresh_surface_attention(index, &surface, cx)
        }));
    }

    fn refresh_surface_attention(
        &mut self,
        index: usize,
        surface: &gpui::Entity<SurfaceState>,
        cx: &mut Context<Self>,
    ) {
        let next = surface
            .read(cx)
            .slack_workspace()
            .map(slack_workspace_attention)
            .unwrap_or(SlackWorkspaceAttention::None);
        if self.attention[index] != next {
            self.attention[index] = next;
            cx.notify();
        }
    }

    pub(super) fn take_pending_selection(&mut self) -> Option<ChatTeamId> {
        self.pending_selection.take()
    }

    fn request_selection(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.add_menu_open = false;
        self.add_menu_selected_index = None;
        self.add_menu_focus_pending = false;
        self.add_menu_restore_button_focus = false;
        let item = &self.items[index];
        window.focus(&item.focus_handle, cx);
        if self.selected_team_id != item.team_id {
            self.selected_team_id = item.team_id.clone();
            self.pending_selection = Some(item.team_id.clone());
            self.scroll_handle
                .scroll_to_item(index, ScrollStrategy::Nearest);
            cx.notify();
        }
        window.refresh();
    }
}
