use super::{
    next_generation, spawn_timer_task_for_entity, Context, SlackConversationFilesCache,
    SlackConversationFilesFilter, SlackConversationFilesSort, SlackMainTab, SurfaceState,
    SLACK_CONVERSATION_FILES_DEBOUNCE, SLACK_CONVERSATION_FILES_PAGINATION_THRESHOLD,
};

impl SurfaceState {
    pub(crate) fn submit_slack_conversation_files_search(&mut self, cx: &mut Context<Self>) {
        self.slack_conversation_files_debounce_generation = next_generation(
            self.slack_conversation_files_debounce_generation,
            "conversation Files debounce",
        );
        self.commit_slack_conversation_files_search(cx);
    }

    pub(in crate::ui::surface::state) fn set_slack_conversation_files_search_query(
        &mut self,
        query: String,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_files_search_query == query {
            return;
        }
        self.slack_conversation_files_search_query = query.clone();
        self.slack_conversation_files_debounce_generation = next_generation(
            self.slack_conversation_files_debounce_generation,
            "conversation Files debounce",
        );
        let generation = self.slack_conversation_files_debounce_generation;
        spawn_timer_task_for_entity(
            (generation, query),
            SLACK_CONVERSATION_FILES_DEBOUNCE,
            cx,
            |this, (generation, query), cx| {
                if this.slack_active_tab != SlackMainTab::FilesLinks
                    || this.slack_conversation_files_debounce_generation != generation
                    || this.slack_conversation_files_search_query != query
                {
                    return;
                }
                this.commit_slack_conversation_files_search(cx);
            },
        );
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn commit_slack_conversation_files_search(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let query = self
            .slack_conversation_files_search_query
            .trim()
            .to_string();
        if self.slack_conversation_files_committed_query == query
            && self.slack_conversation_files_snapshot.is_some()
        {
            return;
        }
        let was_searching = !self.slack_conversation_files_committed_query.is_empty();
        let is_searching = !query.is_empty();
        self.slack_conversation_files_search_query
            .clone_from(&query);
        self.slack_conversation_files_committed_query = query;
        if is_searching && !was_searching {
            self.slack_conversation_files_sort = SlackConversationFilesSort::Relevant;
        } else if !is_searching
            && self.slack_conversation_files_sort == SlackConversationFilesSort::Relevant
        {
            self.slack_conversation_files_sort = SlackConversationFilesSort::Newest;
        }
        self.queue_slack_conversation_files_reload(cx);
    }

    pub(in crate::ui::surface::state) fn queue_slack_conversation_files_reload(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.slack_conversation_files_generation = next_generation(
            self.slack_conversation_files_generation,
            "conversation Files request",
        );
        self.slack_conversation_files_reload_pending = true;
        self.slack_conversation_files_error = None;
        self.slack_conversation_files_menu = None;
        if !self.slack_conversation_files_loading {
            self.start_pending_slack_conversation_files_reload(cx);
        } else {
            cx.notify();
        }
    }

    pub(in crate::ui::surface::state) fn start_pending_slack_conversation_files_reload(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.slack_conversation_files_reload_pending = false;
        self.slack_conversation_files_snapshot = None;
        self.slack_conversation_files_cache = SlackConversationFilesCache::default();
        self.begin_slack_conversation_files_load(Some(1), Some(1), Some(1), cx);
    }

    pub(crate) fn retry_slack_conversation_files(&mut self, cx: &mut Context<Self>) {
        if self.slack_conversation_files_loading {
            return;
        }
        if self
            .slack_conversation_files_cache
            .rows(self.slack_conversation_files_filter)
            .is_empty()
        {
            self.ensure_slack_conversation_files_filter(cx);
        } else {
            self.load_more_slack_conversation_files(cx);
        }
    }

    pub(crate) fn handle_slack_conversation_files_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_conversation_files_explorer_images(visible_start, visible_end, cx);
        if count.saturating_sub(visible_end) <= SLACK_CONVERSATION_FILES_PAGINATION_THRESHOLD {
            self.load_more_slack_conversation_files(cx);
        }
    }

    pub(in crate::ui::surface::state) fn ensure_slack_conversation_files_filter(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_files_loading {
            return;
        }
        let cache = &self.slack_conversation_files_cache;
        let (files, media, links) = match self.slack_conversation_files_filter {
            SlackConversationFilesFilter::All => (
                (!cache.files_loaded).then_some(1),
                (!cache.media_loaded).then_some(1),
                (!cache.links_loaded).then_some(1),
            ),
            SlackConversationFilesFilter::Files => ((!cache.files_loaded).then_some(1), None, None),
            SlackConversationFilesFilter::Media => (None, (!cache.media_loaded).then_some(1), None),
            SlackConversationFilesFilter::Links => (None, None, (!cache.links_loaded).then_some(1)),
        };
        if files.is_some() || media.is_some() || links.is_some() {
            self.begin_slack_conversation_files_load(files, media, links, cx);
        }
    }

    pub(in crate::ui::surface::state) fn load_more_slack_conversation_files(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_files_loading {
            return;
        }
        let cache = &self.slack_conversation_files_cache;
        let searching = !self.slack_conversation_files_committed_query.is_empty();
        let (files, media, links) = match self.slack_conversation_files_filter {
            SlackConversationFilesFilter::All => (
                cache.files_next_page,
                searching.then_some(cache.media_next_page).flatten(),
                cache.links_next_page,
            ),
            SlackConversationFilesFilter::Files => (cache.files_next_page, None, None),
            SlackConversationFilesFilter::Media => (None, cache.media_next_page, None),
            SlackConversationFilesFilter::Links => (None, None, cache.links_next_page),
        };
        if files.is_some() || media.is_some() || links.is_some() {
            self.begin_slack_conversation_files_load(files, media, links, cx);
        }
    }
}
