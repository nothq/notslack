use std::sync::Arc;

use super::{Context, SlackMainTab, SlackRailView, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    prepare_slack_rich_body, SlackAppDmConversationKey, SlackAppDmLoad, SlackAppDmTab,
    SlackAppDmTabSet, SlackAppHomeViewKey, SlackMainRoute,
};
use crate::ui::{SlackAppDmDetails, SlackAppHomeView};

struct SlackAppDmDetailsRequest {
    workspace_api: Arc<dyn WorkspaceApi>,
    key: SlackAppDmConversationKey,
    app_id: Option<String>,
    bot_id: Option<String>,
}

struct SlackAppHomeViewRequest {
    workspace_api: Arc<dyn WorkspaceApi>,
    key: SlackAppHomeViewKey,
}

impl SurfaceState {
    /// App tabs for the conversation header, or `None` when the current
    /// conversation is not a DM with an app.
    pub(crate) fn slack_app_dm_tab_set(&self) -> Option<SlackAppDmTabSet> {
        self.slack_workspace()?.dm_app.as_ref()?;
        let home = self
            .slack_app_dm
            .current_details()
            .and_then(SlackAppDmLoad::loaded)
            .is_some_and(|details| details.home_tab_enabled());
        Some(SlackAppDmTabSet { home })
    }

    /// The app tab replacing the message list, if Home or About is showing.
    pub(crate) fn slack_app_dm_panel_tab(&self) -> Option<SlackAppDmTab> {
        let tab = self.slack_app_dm.active_tab;
        (tab != SlackAppDmTab::Messages
            && self.slack_active_tab == SlackMainTab::Messages
            && self.slack_main_route == SlackMainRoute::Conversation
            && matches!(
                self.slack_active_rail_view,
                SlackRailView::Home | SlackRailView::Dms
            )
            && self
                .slack_workspace()
                .is_some_and(|workspace| workspace.dm_app.is_some()))
        .then_some(tab)
    }

    /// Tracks the current conversation: resets the app tab to Messages when
    /// the conversation changes and lazily loads app details for app DMs.
    pub(crate) fn sync_slack_app_dm_context(&mut self, cx: &mut Context<Self>) {
        let key = self.slack_workspace().and_then(|workspace| {
            (workspace.dm_app.is_some()
                && !workspace.team_id.is_empty()
                && !workspace.conversation_id.is_empty())
            .then(|| (workspace.team_id.clone(), workspace.conversation_id.clone()))
        });
        if key != self.slack_app_dm.conversation {
            self.slack_app_dm.conversation = key.clone();
            if self.slack_app_dm.active_tab != SlackAppDmTab::Messages {
                self.slack_app_dm.active_tab = SlackAppDmTab::Messages;
                cx.notify();
            }
        }
        if let Some(key) = key {
            self.ensure_slack_app_dm_details(key, cx);
        }
    }

    pub(crate) fn reset_slack_app_dm_tab(&mut self) {
        self.slack_app_dm.active_tab = SlackAppDmTab::Messages;
    }

    pub(crate) fn select_slack_app_dm_tab(&mut self, tab: SlackAppDmTab, cx: &mut Context<Self>) {
        self.select_slack_tab(SlackMainTab::Messages, cx);
        if tab == SlackAppDmTab::Messages
            || self.slack_active_tab != SlackMainTab::Messages
            || self
                .slack_workspace()
                .is_none_or(|workspace| workspace.dm_app.is_none())
        {
            return;
        }
        self.slack_app_dm.active_tab = tab;
        self.slack_composer_focused = false;
        if tab == SlackAppDmTab::Home {
            self.ensure_slack_app_home_view(cx);
        }
        self.queue_slack_app_dm_images(cx);
        cx.notify();
    }

    fn queue_slack_app_dm_images(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.slack_app_dm_panel_tab() else {
            return;
        };
        let mut urls = Vec::new();
        match tab {
            SlackAppDmTab::Messages => {}
            SlackAppDmTab::About => {
                let details = self
                    .slack_app_dm
                    .current_details()
                    .and_then(SlackAppDmLoad::loaded);
                urls.extend(
                    details
                        .and_then(|details| details.profile.as_ref()?.icon_url.clone())
                        .or_else(|| {
                            self.slack_workspace()?
                                .dm_app
                                .as_ref()?
                                .avatar_image_url
                                .clone()
                        }),
                );
            }
            SlackAppDmTab::Home => {
                if let Some(body) = self
                    .current_slack_app_home_view_key()
                    .and_then(|key| self.slack_app_dm.home_views.get(&key))
                    .and_then(SlackAppDmLoad::loaded)
                    .and_then(Option::as_ref)
                {
                    urls.extend(body.remote_image_urls().map(|url| url.to_string()));
                }
            }
        }
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    fn ensure_slack_app_dm_details(
        &mut self,
        key: SlackAppDmConversationKey,
        cx: &mut Context<Self>,
    ) {
        if self.slack_app_dm.details.contains_key(&key) {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let Some(dm_app) = self
            .slack_workspace()
            .and_then(|workspace| workspace.dm_app.clone())
        else {
            return;
        };
        self.slack_app_dm
            .details
            .insert(key.clone(), SlackAppDmLoad::Loading);
        self.spawn_background_task(
            SlackAppDmDetailsRequest {
                workspace_api,
                key,
                app_id: dm_app.app_id,
                bot_id: dm_app.bot_id,
            },
            cx,
            |request: SlackAppDmDetailsRequest| {
                let result = request.workspace_api.load_slack_app_dm_details(
                    &request.key.1,
                    request.app_id.as_deref(),
                    request.bot_id.as_deref(),
                );
                (request.key, result)
            },
            |this, (key, result), cx| {
                this.finish_slack_app_dm_details_load(key, result, cx);
            },
        );
    }

    fn finish_slack_app_dm_details_load(
        &mut self,
        key: SlackAppDmConversationKey,
        result: Result<SlackAppDmDetails, String>,
        cx: &mut Context<Self>,
    ) {
        let load = match result {
            Ok(details) => SlackAppDmLoad::Loaded(Arc::new(details)),
            Err(error) => SlackAppDmLoad::Failed(error.into()),
        };
        self.slack_app_dm.details.insert(key.clone(), load);
        // Results for a conversation the user already left stay cached but
        // must not change what is on screen.
        if self.slack_app_dm.conversation.as_ref() != Some(&key) {
            return;
        }
        if self.slack_app_dm.active_tab == SlackAppDmTab::Home {
            self.ensure_slack_app_home_view(cx);
        }
        self.queue_slack_app_dm_images(cx);
        cx.notify();
    }

    /// The current app DM's Home view state: `None` while app details are
    /// loading or when no view id is known.
    pub(crate) fn current_slack_app_home_view_key(&self) -> Option<SlackAppHomeViewKey> {
        let (team_id, _) = self.slack_app_dm.conversation.as_ref()?;
        let details = self.slack_app_dm.current_details()?.loaded()?;
        Some((team_id.clone(), details.home_view_id()?.to_string()))
    }

    fn ensure_slack_app_home_view(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.current_slack_app_home_view_key() else {
            return;
        };
        if matches!(
            self.slack_app_dm.home_views.get(&key),
            Some(SlackAppDmLoad::Loading | SlackAppDmLoad::Loaded(_))
        ) {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.slack_app_dm
            .home_views
            .insert(key.clone(), SlackAppDmLoad::Loading);
        self.spawn_background_task(
            SlackAppHomeViewRequest { workspace_api, key },
            cx,
            |request: SlackAppHomeViewRequest| {
                let result = request
                    .workspace_api
                    .load_slack_app_home_view(&request.key.1);
                (request.key, result)
            },
            |this, (key, result), cx| {
                this.finish_slack_app_home_view_load(key, result, cx);
            },
        );
    }

    fn finish_slack_app_home_view_load(
        &mut self,
        key: SlackAppHomeViewKey,
        result: Result<SlackAppHomeView, String>,
        cx: &mut Context<Self>,
    ) {
        let load = match result {
            Ok(view) => {
                let id_stem = format!("slack-app-home-{}", view.view_id);
                SlackAppDmLoad::Loaded(
                    view.body
                        .map(|body| Arc::new(prepare_slack_rich_body(&body, &id_stem)))
                        .filter(|body| !body.blocks.is_empty()),
                )
            }
            Err(error) => SlackAppDmLoad::Failed(error.into()),
        };
        self.slack_app_dm.home_views.insert(key.clone(), load);
        if self.current_slack_app_home_view_key().as_ref() == Some(&key) {
            self.queue_slack_app_dm_images(cx);
            cx.notify();
        }
    }
}
