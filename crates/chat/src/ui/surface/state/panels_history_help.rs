use super::{Context, SurfaceState};

impl SurfaceState {
    pub(crate) fn open_slack_search_panel(&mut self, cx: &mut Context<Self>) {
        self.open_slack_search(cx);
    }
}
