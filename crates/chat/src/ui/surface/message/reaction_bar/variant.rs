use crate::ui::surface::{
    div, img, px, AnyElement, IntoElement, ParentElement, SlackReactionVariantRow, Styled,
    SurfaceState,
};

impl SurfaceState {
    pub(crate) fn render_slack_reaction_variant(
        &self,
        variant: &SlackReactionVariantRow,
    ) -> AnyElement {
        let Some(cache_key) = variant.image_cache_key.as_ref() else {
            return div()
                .h(px(16.0))
                .text_size(px(15.0))
                .line_height(px(16.0))
                .child(variant.display.clone())
                .into_any_element();
        };
        if let Some(image) = self.slack_remote_images.get(cache_key.as_ref()).cloned() {
            return div()
                .size(px(16.0))
                .flex_none()
                .overflow_hidden()
                .child(img(image).size(px(16.0)))
                .into_any_element();
        }
        div()
            .size(px(16.0))
            .flex_none()
            .text_size(px(15.0))
            .line_height(px(16.0))
            .flex()
            .items_center()
            .justify_center()
            .child(variant.display.clone())
            .into_any_element()
    }
}
