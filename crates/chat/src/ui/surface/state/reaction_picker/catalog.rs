use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::ui::surface::{
    spawn_background_task_for_entity, Context, SlackPreparedReactionPickerCatalog,
    SlackReactionCatalogLoad, SlackReactionPickerEmoji, SlackReactionPickerEmojiPresentation,
    SlackReactionPickerSelection, SlackReactionSkinToneSupport, SurfaceState,
};
use crate::ui::{SlackCustomEmoji, SlackCustomEmojiAsset, SlackReactionCatalogSnapshot};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn sync_slack_reaction_catalog_team(
        &mut self,
        team_id: &str,
    ) {
        let team_changed = self.slack_reaction_picker_catalog.sync_team(team_id);
        if team_changed {
            self.slack_reaction_catalog_generation = self
                .slack_reaction_catalog_generation
                .checked_add(1)
                .expect("Slack reaction catalog generation overflowed");
            self.slack_reaction_catalog_request = None;
            self.slack_reaction_catalog_error = None;
        }
    }

    pub(in crate::ui::surface) fn reset_slack_reaction_catalog(&mut self) {
        self.slack_reaction_catalog_generation = self
            .slack_reaction_catalog_generation
            .checked_add(1)
            .expect("Slack reaction catalog generation overflowed");
        self.slack_reaction_catalog_request = None;
        self.slack_reaction_catalog_error = None;
        self.slack_reaction_picker_catalog = Default::default();
    }

    pub(in crate::ui::surface::state) fn begin_slack_reaction_catalog_load(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_reaction_catalog
            || self.slack_reaction_catalog_request.is_some()
        {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            return;
        };
        self.sync_slack_reaction_catalog_team(&team_id);
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_reaction_catalog_error =
                Some("Custom emoji require a connected Slack workspace.".into());
            return;
        };
        self.slack_reaction_catalog_generation = self
            .slack_reaction_catalog_generation
            .checked_add(1)
            .expect("Slack reaction catalog generation overflowed");
        let request = SlackReactionCatalogLoad {
            generation: self.slack_reaction_catalog_generation,
            team_id,
        };
        self.slack_reaction_catalog_request = Some(request.clone());
        self.slack_reaction_catalog_error = None;
        spawn_background_task_for_entity(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| {
                let result = workspace_api.load_slack_reaction_catalog();
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_reaction_catalog_load(request, result, cx);
            },
        );
    }

    fn finish_slack_reaction_catalog_load(
        &mut self,
        request: SlackReactionCatalogLoad,
        result: Result<Arc<SlackReactionCatalogSnapshot>, String>,
        cx: &mut Context<Self>,
    ) {
        let request_is_current = self.slack_reaction_catalog_generation == request.generation
            && self
                .slack_reaction_catalog_request
                .as_ref()
                .is_some_and(|current| current == &request)
            && self
                .slack_workspace()
                .is_some_and(|workspace| workspace.team_id == request.team_id);
        if !request_is_current {
            return;
        }
        self.slack_reaction_catalog_request = None;
        match result {
            Ok(snapshot) if snapshot.team_id == request.team_id => {
                let catalog_changed = self.slack_reaction_picker_catalog.install(
                    &snapshot.team_id,
                    snapshot.revision,
                    || Self::prepare_slack_reaction_catalog(&snapshot),
                );
                self.slack_reaction_catalog_error = None;
                if catalog_changed {
                    self.rebuild_open_slack_reaction_picker();
                }
            }
            Ok(_) => {
                self.slack_reaction_catalog_error =
                    Some("Slack returned an emoji catalog for a different workspace.".into());
            }
            Err(error) => {
                self.slack_reaction_catalog_error = Some(error.into());
            }
        }
        cx.notify();
    }

    fn prepare_slack_reaction_catalog(
        snapshot: &SlackReactionCatalogSnapshot,
    ) -> SlackPreparedReactionPickerCatalog {
        let custom_presentations = Self::resolve_slack_custom_emoji_presentations(&snapshot.custom);
        let custom = snapshot
            .custom
            .iter()
            .map(|custom| {
                let presentation = custom_presentations
                    .get(custom.name.as_str())
                    .expect("validated Slack custom emoji must have a resolved presentation")
                    .clone();
                Self::slack_custom_reaction_picker_emoji(custom, presentation)
            })
            .collect::<Vec<_>>();
        let custom_by_name = custom
            .iter()
            .map(|emoji| (emoji.name.as_ref(), emoji))
            .collect::<HashMap<_, _>>();
        let mut seen = HashSet::new();
        let frequent = snapshot
            .frequent
            .iter()
            .filter(|name| seen.insert(name.as_str()))
            .filter_map(|name| {
                Self::slack_reaction_picker_emoji_named(name).or_else(|| {
                    custom_by_name
                        .get(name.as_str())
                        .map(|emoji| (*emoji).clone())
                })
            })
            .collect::<Vec<_>>();
        SlackPreparedReactionPickerCatalog {
            frequent: Arc::from(frequent),
            custom: Arc::from(custom),
        }
    }

    fn resolve_slack_custom_emoji_presentations(
        custom: &[SlackCustomEmoji],
    ) -> HashMap<&str, SlackReactionPickerEmojiPresentation> {
        let by_name = custom
            .iter()
            .map(|emoji| (emoji.name.as_str(), &emoji.asset))
            .collect::<HashMap<_, _>>();
        let mut resolved = custom
            .iter()
            .filter_map(|emoji| match &emoji.asset {
                SlackCustomEmojiAsset::ImageUrl(url) => Some((
                    emoji.name.as_str(),
                    SlackReactionPickerEmojiPresentation::RemoteImage {
                        url: url.clone().into(),
                    },
                )),
                SlackCustomEmojiAsset::Alias(_) => None,
            })
            .collect::<HashMap<_, _>>();

        for emoji in custom {
            if resolved.contains_key(emoji.name.as_str()) {
                continue;
            }
            let mut current = emoji.name.as_str();
            let mut path = Vec::new();
            let presentation = loop {
                if let Some(presentation) = resolved.get(current) {
                    break presentation.clone();
                }
                assert!(
                    path.len() < custom.len(),
                    "validated Slack custom emoji aliases must resolve within the catalog bound"
                );
                let SlackCustomEmojiAsset::Alias(target) = by_name
                    .get(current)
                    .expect("validated Slack custom emoji alias target must exist")
                else {
                    unreachable!("image-backed Slack custom emoji must already be resolved");
                };
                path.push(current);
                if let Some(emoji) = Self::slack_reaction_picker_emoji_named(target) {
                    break emoji.presentation;
                }
                current = target;
            };
            for alias in path {
                resolved.insert(alias, presentation.clone());
            }
        }
        resolved
    }

    fn slack_custom_reaction_picker_emoji(
        custom: &SlackCustomEmoji,
        presentation: SlackReactionPickerEmojiPresentation,
    ) -> SlackReactionPickerEmoji {
        SlackReactionPickerEmoji {
            name: custom.name.clone().into(),
            search_key: custom.name.to_ascii_lowercase().into(),
            presentation,
            accessibility_label: format!("React with :{}:", custom.name).into(),
            skin_tone_support: SlackReactionSkinToneSupport::None,
        }
    }

    fn rebuild_open_slack_reaction_picker(&mut self) {
        let Some(picker) = self.slack_reaction_picker.as_ref() else {
            return;
        };
        let source = picker.source();
        let selection = SlackReactionPickerSelection {
            query: picker.query.clone(),
            category_index: picker.category_index,
        };
        self.slack_reaction_picker = Some(self.slack_reaction_picker_state(source, selection));
    }
}
