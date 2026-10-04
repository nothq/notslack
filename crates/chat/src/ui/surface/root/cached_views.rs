use crate::ui::surface::{
    Context, Entity, IntoElement, Render, SlackCachedSurfaceRegion, SlackCachedSurfaceView,
    SlackSearchOverlayView, SlackSearchResultsView, SurfaceState, Window,
};
impl SlackCachedSurfaceView {
    pub(in crate::ui::surface) fn new(
        surface: Entity<SurfaceState>,
        region: SlackCachedSurfaceRegion,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&surface, |_, _, cx| cx.notify()).detach();
        Self {
            surface: surface.downgrade(),
            region,
        }
    }
}

impl Render for SlackCachedSurfaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = self
            .surface
            .upgrade()
            .expect("cached Slack region lost its surface");
        surface.update(cx, |surface, cx| match self.region {
            SlackCachedSurfaceRegion::History => surface.render_slack_history_bar(cx),
            SlackCachedSurfaceRegion::Rail => {
                let workspace = surface
                    .slack_workspace
                    .clone()
                    .expect("cached Slack rail requires a workspace");
                surface.render_slack_rail(&workspace, cx)
            }
            SlackCachedSurfaceRegion::Sidebar => {
                let workspace = surface
                    .slack_workspace
                    .clone()
                    .expect("cached Slack sidebar requires a workspace");
                surface.render_slack_sidebar(&workspace, cx)
            }
            SlackCachedSurfaceRegion::Conversation => surface.render_slack_conversation_content(cx),
        })
    }
}

impl SlackSearchOverlayView {
    pub(in crate::ui::surface) fn new(
        surface: Entity<SurfaceState>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&surface, |_, _, cx| cx.notify()).detach();
        Self {
            surface: surface.downgrade(),
        }
    }
}

impl Render for SlackSearchOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = self
            .surface
            .upgrade()
            .expect("Slack search overlay lost its surface");
        surface.update(cx, |surface, cx| surface.render_slack_search_layer(cx))
    }
}

impl SlackSearchResultsView {
    pub(in crate::ui::surface) fn new(
        surface: Entity<SurfaceState>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&surface, |_, _, cx| cx.notify()).detach();
        Self {
            surface: surface.downgrade(),
        }
    }
}

impl Render for SlackSearchResultsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = self
            .surface
            .upgrade()
            .expect("Slack search results lost their surface");
        surface.update(cx, |surface, cx| {
            surface.render_slack_search_results_surface(cx)
        })
    }
}
