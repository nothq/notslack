use gpui::{Context, Entity, Subscription};

use super::{workspace_host::PendingSlackNotificationNavigation, SurfaceState};
use crate::model::{
    ChatSurfaceEvent, ChatTeamId, SlackNotificationTeamBadge, SlackNotificationWindowContext,
};

pub(super) struct ChatSelectedSurface<'a> {
    pub(super) team_id: Option<ChatTeamId>,
    pub(super) selection_generation: u64,
    pub(super) surface: Option<&'a Entity<SurfaceState>>,
    pub(super) root_active: bool,
}

pub struct ChatEventSource {
    selected_team_id: Option<ChatTeamId>,
    selection_generation: u64,
    root_active: bool,
    selected_surface: Option<gpui::WeakEntity<SurfaceState>>,
    notification_window_context: Option<SlackNotificationWindowContext>,
    notification_team_badge: Option<SlackNotificationTeamBadge>,
    pending_notification: Option<PendingSlackNotificationNavigation>,
    surface_subscriptions: Vec<Subscription>,
}

impl ChatEventSource {
    pub(super) fn new() -> Self {
        Self {
            selected_team_id: None,
            selection_generation: 0,
            root_active: false,
            selected_surface: None,
            notification_window_context: None,
            notification_team_badge: None,
            pending_notification: None,
            surface_subscriptions: Vec::new(),
        }
    }

    pub(super) fn attach_surface(
        &mut self,
        team_id: Option<ChatTeamId>,
        surface: &Entity<SurfaceState>,
        cx: &mut Context<Self>,
    ) {
        let emitted_team_id = team_id.clone();
        self.surface_subscriptions.push(cx.subscribe(
            surface,
            move |this, source, event: &ChatSurfaceEvent, cx| {
                if emitted_team_id
                    .as_ref()
                    .is_none_or(|team_id| this.selected_team_id.as_ref() == Some(team_id))
                {
                    this.refresh_notification_window_context_from(&source, cx);
                    this.dispatch_pending_notification_to(&source, cx);
                    cx.emit(event.clone());
                }
            },
        ));
        self.surface_subscriptions
            .push(cx.observe(surface, move |this, source, cx| {
                if team_id
                    .as_ref()
                    .is_none_or(|team_id| this.selected_team_id.as_ref() == Some(team_id))
                {
                    this.refresh_notification_window_context_from(&source, cx);
                    this.dispatch_pending_notification_to(&source, cx);
                }
            }));
    }

    pub(super) fn set_selected_surface(
        &mut self,
        selected: ChatSelectedSurface<'_>,
        cx: &mut Context<Self>,
    ) {
        let ChatSelectedSurface {
            team_id,
            selection_generation,
            surface,
            root_active,
        } = selected;
        if self.selected_team_id != team_id || self.selection_generation != selection_generation {
            self.pending_notification = None;
        }
        self.selected_team_id = team_id;
        self.selection_generation = selection_generation;
        self.root_active = root_active;
        self.selected_surface = surface.map(|surface| surface.downgrade());
        self.refresh_notification_state(cx);
        self.dispatch_pending_notification(cx);
    }

    pub(super) fn queue_notification_navigation(
        &mut self,
        pending: PendingSlackNotificationNavigation,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.selected_team_id.as_ref() != Some(&pending.team_id)
            || self.selection_generation != pending.selection_generation
        {
            return Err("Slack notification selection changed before navigation".to_string());
        }
        self.pending_notification = Some(pending);
        self.dispatch_pending_notification(cx);
        Ok(())
    }

    pub fn slack_notification_window_context(&self) -> Option<SlackNotificationWindowContext> {
        self.notification_window_context.clone()
    }

    pub fn slack_notification_team_badge(&self) -> Option<SlackNotificationTeamBadge> {
        self.notification_team_badge.clone()
    }

    fn refresh_notification_state(&mut self, cx: &mut Context<Self>) {
        let selected_surface = self
            .selected_surface
            .as_ref()
            .and_then(gpui::WeakEntity::upgrade);
        match selected_surface {
            Some(surface) => self.refresh_notification_window_context_from(&surface, cx),
            None => self.set_loading_notification_state(cx),
        }
    }

    fn refresh_notification_window_context_from(
        &mut self,
        surface: &Entity<SurfaceState>,
        cx: &mut Context<Self>,
    ) {
        let Some(selected_team_id) = self.selected_team_id.as_ref() else {
            let context_changed = self.notification_window_context.take().is_some();
            let badge_changed = self.notification_team_badge.take().is_some();
            if context_changed {
                cx.emit(ChatSurfaceEvent::NotificationWindowContextChanged(None));
            }
            if context_changed || badge_changed {
                cx.notify();
            }
            return;
        };
        let surface = surface.read(cx);
        let next_context = surface
            .slack_notification_window_context()
            .filter(|context| context.team_id == selected_team_id.as_str())
            .map(|mut context| {
                context.active_surface = self.root_active;
                context
            })
            .unwrap_or_else(|| SlackNotificationWindowContext {
                team_id: selected_team_id.as_str().to_string(),
                active_surface: self.root_active,
                visible_targets: Vec::new(),
            });
        let next_badge = surface
            .slack_notification_team_badge()
            .filter(|badge| badge.team_id == selected_team_id.as_str());
        let context_changed = self.notification_window_context.as_ref() != Some(&next_context);
        let badge_changed = self.notification_team_badge != next_badge;
        if context_changed || badge_changed {
            self.notification_window_context = Some(next_context);
            self.notification_team_badge = next_badge;
            if context_changed {
                cx.emit(ChatSurfaceEvent::NotificationWindowContextChanged(
                    self.notification_window_context.clone(),
                ));
            }
            cx.notify();
        }
    }

    fn set_loading_notification_state(&mut self, cx: &mut Context<Self>) {
        let next_context =
            self.selected_team_id
                .as_ref()
                .map(|team_id| SlackNotificationWindowContext {
                    team_id: team_id.as_str().to_string(),
                    active_surface: self.root_active,
                    visible_targets: Vec::new(),
                });
        let context_changed = self.notification_window_context != next_context;
        let badge_changed = self.notification_team_badge.is_some();
        if context_changed || badge_changed {
            self.notification_window_context = next_context;
            self.notification_team_badge = None;
            if context_changed {
                cx.emit(ChatSurfaceEvent::NotificationWindowContextChanged(
                    self.notification_window_context.clone(),
                ));
            }
            cx.notify();
        }
    }

    fn dispatch_pending_notification(&mut self, cx: &mut Context<Self>) {
        let selected_surface = self
            .selected_surface
            .as_ref()
            .and_then(gpui::WeakEntity::upgrade);
        if let Some(surface) = selected_surface {
            self.dispatch_pending_notification_to(&surface, cx);
        }
    }

    fn dispatch_pending_notification_to(
        &mut self,
        surface: &Entity<SurfaceState>,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.pending_notification.as_ref() else {
            return;
        };
        if self.selected_team_id.as_ref() != Some(&pending.team_id)
            || self.selection_generation != pending.selection_generation
            || surface
                .read(cx)
                .slack_workspace()
                .is_none_or(|workspace| workspace.team_id != pending.team_id.as_str())
        {
            return;
        }
        let pending = self
            .pending_notification
            .take()
            .expect("checked pending Slack notification navigation");
        let result = surface.update(cx, |surface, cx| {
            surface.open_slack_notification_target(
                crate::model::SlackNotificationTarget {
                    team_id: pending.team_id.as_str(),
                    conversation_id: &pending.conversation_id,
                    message_timestamp: pending.message_timestamp.as_deref(),
                    thread_timestamp: pending.thread_timestamp.as_deref(),
                    launch_uri: pending.launch_uri.as_deref(),
                },
                cx,
            )
        });
        if let Err(error) = result {
            eprintln!("failed to open selected Slack notification target: {error}");
        }
    }
}
