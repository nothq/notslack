use super::{Context, SurfaceState};
use crate::ui::surface::{
    slack_duration_until_next_minute, SlackActiveMainComposerContext, SlackComposerDestination,
    SlackComposerDraft, SlackComposerDraftKey, SlackMainComposerDraftOwner,
    SlackMainComposerNotice, SlackMainComposerPresentation, SlackMainComposerTarget,
    SlackMainRoute, SlackNewMessageDraftKey, SlackSendDraftSource,
};
use crate::ui::{
    SlackConversationKind, SlackConversationSnapshot, SlackDmPeerLocalTimeContext, SlackWorkspace,
};

struct SlackMainComposerPresentationInput<'a> {
    kind: SlackConversationKind,
    conversation_label: &'a str,
    placeholder: &'a str,
    notice: Option<&'a str>,
    notifications_paused: bool,
    dm_peer_local_time_context: Option<&'a SlackDmPeerLocalTimeContext>,
}

impl SurfaceState {
    pub(in crate::ui::surface) fn initialize_slack_main_composer_context(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.slack_active_main_composer_context = self.slack_routed_main_composer_context();
        self.schedule_slack_main_composer_minute_tick(cx);
    }

    pub(in crate::ui::surface::state) fn slack_routed_main_composer_context(
        &self,
    ) -> Option<SlackActiveMainComposerContext> {
        let workspace = self.slack_workspace()?;
        let self_user_id = workspace.self_user_id.clone()?;
        match self.slack_main_route {
            SlackMainRoute::Conversation => {
                let conversation_id = self.slack_conversation_id()?.to_string();
                Some(slack_conversation_composer_context(
                    workspace,
                    self_user_id,
                    conversation_id.clone(),
                    SlackSendDraftSource::Conversation { conversation_id },
                ))
            }
            SlackMainRoute::NewMessage => {
                let destination = self.slack_new_message_destination.as_ref()?;
                if destination.conversation_id.as_ref() != workspace.conversation_id {
                    return None;
                }
                let draft_key = self.slack_new_message_active_draft_key.clone()?;
                let owner = SlackMainComposerDraftOwner::NewMessage(SlackNewMessageDraftKey {
                    team_id: workspace.team_id.clone(),
                    self_user_id: self_user_id.clone(),
                    draft_key: draft_key.clone(),
                });
                Some(SlackActiveMainComposerContext::new(
                    owner,
                    SlackSendDraftSource::NewMessage { draft_key },
                    SlackMainComposerTarget {
                        team_id: workspace.team_id.clone(),
                        self_user_id,
                        conversation_id: workspace.conversation_id.clone(),
                    },
                    slack_main_composer_presentation(SlackMainComposerPresentationInput {
                        kind: destination.kind,
                        conversation_label: destination.label.as_ref(),
                        placeholder: &destination
                            .kind
                            .composer_placeholder(destination.label.as_ref()),
                        notice: None,
                        notifications_paused: false,
                        dm_peer_local_time_context: None,
                    }),
                ))
            }
            SlackMainRoute::AllThreads | SlackMainRoute::Directory => None,
        }
    }

    pub(in crate::ui::surface::state) fn install_slack_routed_main_composer_after_workspace_preparation(
        &mut self,
    ) {
        let next = self.slack_routed_main_composer_context();
        match (
            self.slack_active_main_composer_context.as_ref(),
            next.as_ref(),
        ) {
            (None, _) => {
                assert!(
                    self.slack_composer_files.is_empty(),
                    "workspace preparation must park files before installing a routed composer"
                );
            }
            (Some(current), Some(next)) => {
                assert_eq!(
                    current.owner, next.owner,
                    "workspace preparation cannot replace an unparked main composer owner"
                );
            }
            (Some(_), None) => {
                panic!("workspace preparation cannot remove an unparked main composer owner");
            }
        }
        self.slack_active_main_composer_context = next;
    }

    pub(in crate::ui::surface::state) fn clear_slack_main_composer_after_draft_taken(
        &mut self,
        owner: &SlackMainComposerDraftOwner,
    ) {
        assert!(
            self.slack_active_main_composer_context
                .as_ref()
                .is_some_and(|context| &context.owner == owner),
            "parking a Slack main draft must clear its exact active owner"
        );
        assert!(
            self.slack_composer_files.is_empty(),
            "parking a Slack main draft must move every owned file"
        );
        self.slack_active_main_composer_context = None;
    }

    pub(in crate::ui::surface::state) fn activate_slack_activity_main_composer(
        &mut self,
        context: SlackActiveMainComposerContext,
        cx: &mut Context<Self>,
    ) -> bool {
        let activated = self.swap_slack_main_composer_context(Some(context), cx);
        if activated {
            let _ = self.restore_matching_slack_composer_schedule_recovery(cx);
            self.restore_matching_slack_scheduled_edit_recovery(cx);
        }
        activated
    }

    pub(in crate::ui::surface::state) fn restore_slack_routed_main_composer(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let next = self.slack_routed_main_composer_context();
        self.swap_slack_main_composer_context(next, cx);
        let _ = self.restore_matching_slack_composer_schedule_recovery(cx);
        self.restore_matching_slack_scheduled_edit_recovery(cx);
    }

    pub(in crate::ui::surface::state) fn park_slack_main_composer(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.swap_slack_main_composer_context(None, cx);
    }

    fn swap_slack_main_composer_context(
        &mut self,
        next: Option<SlackActiveMainComposerContext>,
        cx: &mut Context<Self>,
    ) -> bool {
        let owner_unchanged = self
            .slack_active_main_composer_context
            .as_ref()
            .map(|context| &context.owner)
            == next.as_ref().map(|context| &context.owner);
        let source_unchanged = self
            .slack_active_main_composer_context
            .as_ref()
            .map(|context| &context.source)
            == next.as_ref().map(|context| &context.source);
        if !owner_unchanged {
            self.cancel_slack_composer_capture_for_owner_change(cx);
        }
        if self.slack_active_scheduled_edit.is_some() && !source_unchanged {
            self.restore_slack_scheduled_edit_prior_draft(cx);
        }
        if owner_unchanged {
            self.slack_active_main_composer_context = next;
            return true;
        }
        if self.slack_active_scheduled_edit.is_some() {
            self.restore_slack_scheduled_edit_prior_draft(cx);
        }
        if let Some(current) = self.slack_active_main_composer_context.take() {
            let draft = self.take_slack_send_draft();
            self.store_slack_main_composer_owner_draft(current.owner, draft);
        } else {
            assert!(
                self.slack_composer_files.is_empty(),
                "an ownerless Slack main composer cannot retain files"
            );
        }
        let draft = next
            .as_ref()
            .and_then(|context| self.take_slack_main_composer_owner_draft(&context.owner));
        self.slack_active_main_composer_context = next;
        self.restore_slack_send_draft(draft);
        self.slack_composer_focused = false;
        self.slack_error = None;
        true
    }

    pub(in crate::ui::surface::state) fn store_slack_main_composer_owner_draft(
        &mut self,
        owner: SlackMainComposerDraftOwner,
        draft: SlackComposerDraft,
    ) {
        match owner {
            SlackMainComposerDraftOwner::Conversation(key) => {
                self.store_slack_composer_draft(key, draft);
            }
            SlackMainComposerDraftOwner::NewMessage(key) => {
                if let Some(existing) = self.slack_new_message_drafts.get(&key) {
                    assert!(
                        existing.files.is_empty()
                            || (existing.id == draft.id
                                && existing.files.same_identity_and_order(&draft.files)),
                        "storing a Slack new-message draft cannot replace a different file-owning draft"
                    );
                }
                if draft.is_empty() {
                    self.slack_new_message_drafts.remove(&key);
                } else {
                    self.slack_new_message_drafts.insert(key, draft);
                }
            }
        }
    }

    pub(in crate::ui::surface::state) fn take_slack_main_composer_owner_draft(
        &mut self,
        owner: &SlackMainComposerDraftOwner,
    ) -> Option<SlackComposerDraft> {
        match owner {
            SlackMainComposerDraftOwner::Conversation(key) => {
                self.slack_composer_drafts.remove(key)
            }
            SlackMainComposerDraftOwner::NewMessage(key) => {
                self.slack_new_message_drafts.remove(key)
            }
        }
    }
}

pub(in crate::ui::surface::state) fn slack_activity_conversation_composer_context(
    snapshot: &SlackConversationSnapshot,
    self_user_id: String,
    item_key: gpui::SharedString,
) -> SlackActiveMainComposerContext {
    let conversation_id = snapshot.conversation_id.clone();
    let key = SlackComposerDraftKey {
        team_id: snapshot.team_id.clone(),
        self_user_id: self_user_id.clone(),
        destination: SlackComposerDestination::Conversation {
            conversation_id: conversation_id.clone(),
        },
    };
    SlackActiveMainComposerContext::new(
        SlackMainComposerDraftOwner::Conversation(key),
        SlackSendDraftSource::Activity {
            item_key,
            conversation_id: conversation_id.clone(),
        },
        SlackMainComposerTarget {
            team_id: snapshot.team_id.clone(),
            self_user_id,
            conversation_id,
        },
        slack_main_composer_presentation(SlackMainComposerPresentationInput {
            kind: snapshot.channel_kind,
            conversation_label: &snapshot.channel_name,
            placeholder: &slack_composer_placeholder(
                snapshot.channel_kind,
                &snapshot.channel_name,
                &snapshot.composer_placeholder,
            ),
            notice: snapshot.composer_notice.as_deref(),
            notifications_paused: snapshot.peer_notifications_paused,
            dm_peer_local_time_context: snapshot.dm_peer_local_time_context.as_ref(),
        }),
    )
}

pub(in crate::ui::surface::state) fn slack_activity_current_conversation_composer_context(
    workspace: &SlackWorkspace,
    self_user_id: String,
    item_key: gpui::SharedString,
) -> SlackActiveMainComposerContext {
    let snapshot = workspace.conversation_snapshot();
    slack_activity_conversation_composer_context(&snapshot, self_user_id, item_key)
}

fn slack_conversation_composer_context(
    workspace: &SlackWorkspace,
    self_user_id: String,
    conversation_id: String,
    source: SlackSendDraftSource,
) -> SlackActiveMainComposerContext {
    let key = SlackComposerDraftKey {
        team_id: workspace.team_id.clone(),
        self_user_id: self_user_id.clone(),
        destination: SlackComposerDestination::Conversation {
            conversation_id: conversation_id.clone(),
        },
    };
    SlackActiveMainComposerContext::new(
        SlackMainComposerDraftOwner::Conversation(key),
        source,
        SlackMainComposerTarget {
            team_id: workspace.team_id.clone(),
            self_user_id,
            conversation_id,
        },
        slack_main_composer_presentation(SlackMainComposerPresentationInput {
            kind: workspace.channel_kind,
            conversation_label: &workspace.channel_name,
            placeholder: &slack_composer_placeholder(
                workspace.channel_kind,
                &workspace.channel_name,
                &workspace.composer_placeholder,
            ),
            notice: workspace.composer_notice.as_deref(),
            notifications_paused: workspace.peer_notifications_paused,
            dm_peer_local_time_context: workspace.dm_peer_local_time_context.as_ref(),
        }),
    )
}

fn slack_main_composer_presentation(
    input: SlackMainComposerPresentationInput<'_>,
) -> SlackMainComposerPresentation {
    let SlackMainComposerPresentationInput {
        kind,
        conversation_label,
        placeholder,
        notice,
        notifications_paused,
        dm_peer_local_time_context,
    } = input;
    let notice = if notifications_paused {
        Some(SlackMainComposerNotice::NotificationsPaused {
            conversation_label: conversation_label.to_string().into(),
        })
    } else if let Some(notice) = notice {
        Some(SlackMainComposerNotice::Message(notice.to_string().into()))
    } else {
        dm_peer_local_time_context.map(|context| SlackMainComposerNotice::PeerLocalTime {
            conversation_label: context.conversation_label.clone().into(),
            timezone: context.timezone,
        })
    };
    SlackMainComposerPresentation {
        conversation_label: conversation_label.to_string().into(),
        placeholder: placeholder.to_string().into(),
        private_channel: kind == SlackConversationKind::PrivateChannel,
        notice,
    }
}

impl SurfaceState {
    fn schedule_slack_main_composer_minute_tick(&mut self, cx: &mut Context<Self>) {
        self.spawn_timer_task(
            (),
            slack_duration_until_next_minute(),
            cx,
            |this, (), cx| {
                if this
                    .slack_active_main_composer_context
                    .as_ref()
                    .and_then(|context| context.presentation.notice.as_ref())
                    .is_some_and(|notice| {
                        matches!(notice, SlackMainComposerNotice::PeerLocalTime { .. })
                    })
                {
                    cx.notify();
                }
                this.schedule_slack_main_composer_minute_tick(cx);
            },
        );
    }
}

fn slack_composer_placeholder(
    kind: SlackConversationKind,
    conversation_label: &str,
    source: &str,
) -> String {
    if source == "Jot something down" {
        return source.to_string();
    }
    source
        .strip_prefix("Message to ")
        .map(|channel| format!("Message {channel}"))
        .unwrap_or_else(|| {
            if source.is_empty() {
                kind.composer_placeholder(conversation_label)
            } else {
                source.to_string()
            }
        })
}
