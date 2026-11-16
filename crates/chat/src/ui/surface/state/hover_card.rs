//! The profile card that opens when the pointer rests on a person's name
//! or an @mention, as in Slack.

use std::time::Duration;

use gpui::{Pixels, Point};

use crate::ui::spawn_timer_task_for_entity;
use crate::ui::surface::{Context, SlackHoverCard, SurfaceState};

/// How long the pointer rests before the card opens.
const SLACK_HOVER_CARD_DELAY: Duration = Duration::from_millis(450);

impl SurfaceState {
    /// The pointer moved onto `user_id`'s name, or off every name (`None`).
    pub(crate) fn hover_slack_person(
        &mut self,
        user_id: Option<&str>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(user_id) = user_id else {
            if self.slack_hover_card.take().is_some() {
                cx.notify();
            }
            return;
        };
        if self
            .slack_hover_card
            .as_ref()
            .is_some_and(|card| card.user_id == user_id)
        {
            return;
        }
        self.slack_hover_card_generation = self.slack_hover_card_generation.wrapping_add(1);
        let generation = self.slack_hover_card_generation;
        let had_card = self.slack_hover_card.is_some();
        self.slack_hover_card = Some(SlackHoverCard {
            generation,
            user_id: user_id.to_string(),
            position,
            visible: false,
        });
        if had_card {
            cx.notify();
        }
        spawn_timer_task_for_entity(
            generation,
            SLACK_HOVER_CARD_DELAY,
            cx,
            |this, generation, cx| {
                let Some(card) = this
                    .slack_hover_card
                    .as_mut()
                    .filter(|card| card.generation == generation)
                else {
                    return;
                };
                card.visible = true;
                let user_id = card.user_id.clone();
                this.ensure_slack_hover_profile(user_id, cx);
                cx.notify();
            },
        );
    }

    fn ensure_slack_hover_profile(&mut self, user_id: String, cx: &mut Context<Self>) {
        if self.slack_profile_cache.contains_key(&user_id)
            || !self.slack_workspace_api_capabilities.load_profile
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.spawn_background_task(
            user_id,
            cx,
            move |user_id| {
                let profile = workspace_api.load_slack_profile(&user_id);
                (user_id, profile)
            },
            |this, (user_id, profile), cx| {
                let Ok(profile) = profile else {
                    return;
                };
                if let Some(url) = profile.avatar_image_url.clone() {
                    this.enqueue_slack_remote_image_url(url, cx);
                }
                this.slack_profile_cache.insert(user_id, profile);
                cx.notify();
            },
        );
    }
}
