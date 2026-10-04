use std::{collections::HashSet, rc::Rc, sync::Arc};

use gpui::Entity;
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SurfaceState};
use crate::ui::surface::{
    prepare_slack_sidebar_snapshot, slack_palette, PreparedSlackSidebarSnapshot,
    SlackSidebarSectionChoice, SlackSidebarSectionDialog, WorkspaceApi,
};
use crate::ui::{
    alpha, px, rgb, spawn_background_task_for_entity, SlackSidebarSectionCreateRequest,
    SlackSidebarSectionSort,
};

type SlackSidebarSectionCreateTask = (
    Arc<dyn WorkspaceApi>,
    SlackSidebarSectionCreateRequest,
    String,
    u64,
);

struct SlackSidebarSectionSubmission {
    workspace_api: Arc<dyn WorkspaceApi>,
    request: SlackSidebarSectionCreateRequest,
    conversation_id: String,
    generation: u64,
    collapsed_sections: HashSet<String>,
    muted_conversations: HashSet<String>,
}

impl SurfaceState {
    pub(crate) fn open_slack_sidebar_section_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.subscribe_realtime {
            return;
        }
        let mut seen = HashSet::new();
        let choices = self
            .slack_workspace()
            .into_iter()
            .flat_map(|workspace| workspace.sections.iter())
            .flat_map(|section| section.items.iter())
            .filter(|item| !item.target_id.is_empty() && seen.insert(item.target_id.clone()))
            .enumerate()
            .map(|(source_index, item)| SlackSidebarSectionChoice {
                conversation_id: item.target_id.clone(),
                label: item.label.clone(),
                kind: item.target_kind,
                selected: false,
                source_index,
            })
            .collect();
        self.slack_sidebar_section_dialog = Some(SlackSidebarSectionDialog {
            name: String::new(),
            sort: SlackSidebarSectionSort::Recent,
            choices,
            scroll_handle: gpui::UniformListScrollHandle::new(),
            saving: false,
            error: None,
        });
        self.slack_sidebar_section_generation =
            self.slack_sidebar_section_generation.wrapping_add(1);
        cx.notify();
    }

    pub(crate) fn close_slack_sidebar_section_dialog(&mut self, cx: &mut Context<Self>) {
        if self
            .slack_sidebar_section_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.saving)
        {
            return;
        }
        if self.slack_sidebar_section_dialog.take().is_some() {
            self.slack_sidebar_section_generation =
                self.slack_sidebar_section_generation.wrapping_add(1);
            cx.notify();
        }
    }

    pub(crate) fn toggle_slack_sidebar_section_choice(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(choice) = self
            .slack_sidebar_section_dialog
            .as_mut()
            .filter(|dialog| !dialog.saving)
            .and_then(|dialog| dialog.choices.get_mut(index))
        else {
            return;
        };
        choice.selected = !choice.selected;
        if let Some(dialog) = self.slack_sidebar_section_dialog.as_mut() {
            dialog.error = None;
        }
        cx.notify();
    }

    pub(crate) fn toggle_slack_sidebar_section_sort(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self
            .slack_sidebar_section_dialog
            .as_mut()
            .filter(|dialog| !dialog.saving)
        else {
            return;
        };
        dialog.sort = match dialog.sort {
            SlackSidebarSectionSort::Recent => SlackSidebarSectionSort::Alphabetical,
            SlackSidebarSectionSort::Alphabetical => SlackSidebarSectionSort::Recent,
        };
        dialog.error = None;
        cx.notify();
    }

    pub(crate) fn submit_slack_sidebar_section(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.slack_sidebar_section_create_request(cx) else {
            return;
        };
        let Some(submission) = self.begin_slack_sidebar_section_submission(request, cx) else {
            return;
        };
        self.spawn_slack_sidebar_section_submission(submission, cx);
    }

    fn slack_sidebar_section_create_request(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<SlackSidebarSectionCreateRequest> {
        let dialog = self
            .slack_sidebar_section_dialog
            .as_ref()
            .filter(|dialog| !dialog.saving)?;
        let mut selected = dialog
            .choices
            .iter()
            .filter(|choice| choice.selected)
            .collect::<Vec<_>>();
        match dialog.sort {
            SlackSidebarSectionSort::Recent => {
                selected.sort_by_key(|choice| choice.source_index);
            }
            SlackSidebarSectionSort::Alphabetical => {
                selected.sort_by_cached_key(|choice| choice.label.to_lowercase());
            }
        }
        match SlackSidebarSectionCreateRequest::new(
            dialog.name.clone(),
            selected
                .into_iter()
                .map(|choice| choice.conversation_id.clone())
                .collect(),
            dialog.sort,
        ) {
            Ok(request) => Some(request),
            Err(error) => {
                self.slack_sidebar_section_dialog
                    .as_mut()
                    .expect("validated Slack sidebar section dialog must remain open")
                    .error = Some(error);
                cx.notify();
                None
            }
        }
    }

    fn begin_slack_sidebar_section_submission(
        &mut self,
        request: SlackSidebarSectionCreateRequest,
        cx: &mut Context<Self>,
    ) -> Option<SlackSidebarSectionSubmission> {
        let workspace_api = self.active_slack_workspace_api()?;
        let conversation_id = self
            .slack_workspace()
            .map(|workspace| workspace.conversation_id.clone())?;
        let collapsed_sections = self.slack_collapsed_sections.clone();
        let muted_conversations = self.slack_muted_conversations.clone();
        self.slack_sidebar_section_generation =
            self.slack_sidebar_section_generation.wrapping_add(1);
        let generation = self.slack_sidebar_section_generation;
        let dialog = self
            .slack_sidebar_section_dialog
            .as_mut()
            .expect("Slack sidebar section mutation requires an open dialog");
        dialog.saving = true;
        dialog.error = None;
        cx.notify();
        Some(SlackSidebarSectionSubmission {
            workspace_api,
            request,
            conversation_id,
            generation,
            collapsed_sections,
            muted_conversations,
        })
    }

    fn spawn_slack_sidebar_section_submission(
        &self,
        submission: SlackSidebarSectionSubmission,
        cx: &mut Context<Self>,
    ) {
        let SlackSidebarSectionSubmission {
            workspace_api,
            request,
            conversation_id,
            generation,
            collapsed_sections,
            muted_conversations,
        } = submission;
        spawn_background_task_for_entity(
            (workspace_api, request, conversation_id, generation),
            cx,
            move |(workspace_api, request, conversation_id, generation): SlackSidebarSectionCreateTask| {
                let result = workspace_api
                    .create_slack_sidebar_section(&request, &conversation_id)
                    .map(|snapshot| {
                        prepare_slack_sidebar_snapshot(
                            snapshot,
                            &collapsed_sections,
                            &muted_conversations,
                        )
                    });
                (generation, result)
            },
            |this, (generation, result), cx| {
                this.finish_slack_sidebar_section_submission(generation, result, cx);
            },
        );
    }

    fn finish_slack_sidebar_section_submission(
        &mut self,
        generation: u64,
        result: Result<PreparedSlackSidebarSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.slack_sidebar_section_generation
            || self.slack_sidebar_section_dialog.is_none()
        {
            return;
        }
        match result {
            Ok(prepared) => {
                self.slack_sidebar_section_dialog = None;
                self.apply_prepared_slack_sidebar_snapshot(prepared, cx);
            }
            Err(error) => {
                let dialog = self
                    .slack_sidebar_section_dialog
                    .as_mut()
                    .expect("validated Slack sidebar section dialog must remain open");
                dialog.saving = false;
                dialog.error = Some(error);
                cx.notify();
            }
        }
    }

    pub(crate) fn slack_sidebar_section_name_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let dialog = self
            .slack_sidebar_section_dialog
            .as_ref()
            .expect("Slack sidebar section name input requires an open dialog");
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if let Some(dialog) = surface.slack_sidebar_section_dialog.as_mut() {
                        dialog.name = value;
                        dialog.error = None;
                        cx.notify();
                    }
                })
                .ok();
        });
        let props = TextInputProps::single_line(dialog.name.clone())
            .placeholder("Section name")
            .style(self.slack_sidebar_section_name_input_style())
            .accessibility(
                self.slack_sidebar_section_name_accessibility_id.clone(),
                "Section name",
            )
            .on_change(on_change)
            .on_submit(self.slack_sidebar_section_submit_action(cx))
            .on_escape(self.slack_sidebar_section_close_action(cx));
        self.slack_sidebar_section_name_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_sidebar_section_name_input.clone()
    }

    fn slack_sidebar_section_submit_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| surface.submit_slack_sidebar_section(cx))
                .ok();
        })
    }

    fn slack_sidebar_section_close_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.close_slack_sidebar_section_dialog(cx)
                })
                .ok();
        })
    }

    fn slack_sidebar_section_name_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(38.0),
            min_height: px(38.0),
            padding_x: px(10.0),
            padding_y: px(8.0),
            radius: px(6.0),
            background: rgb(palette.composer_bg).into(),
            border: rgb(palette.composer_border).into(),
            focused_border: rgb(palette.composer_focused_border).into(),
            text: rgb(palette.main_text).into(),
            placeholder: rgb(palette.composer_placeholder).into(),
            selection: alpha(palette.link, 0.28),
            caret: rgb(palette.main_text).into(),
            font_size: px(15.0),
            line_height: px(22.0),
            font_family: Some("Lato".into()),
        }
    }
}
