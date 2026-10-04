use crate::ui::surface::{
    SlackMessageBody, SlackMessageBodyAction, SlackMessageBodyBlock, SlackMessageBodyBlockKind,
    SlackMessageBodyContextElement, SlackMessageBodyImage, SlackMessageBodyLink,
    SlackMessageBodyStyleRange, SlackMessageBodyTarget, SlackMessageBodyText,
    SlackMessageElementIds, SlackMessageTextStyle,
};

pub(super) struct SlackMessageBodyBuilder<'a> {
    id_stem: &'a str,
    text: String,
    links: Vec<SlackMessageBodyLink>,
    blocks: Vec<SlackMessageBodyBlock>,
}

impl<'a> SlackMessageBodyBuilder<'a> {
    pub(super) fn new(id_stem: &'a str) -> Self {
        Self {
            id_stem,
            text: String::new(),
            links: Vec::new(),
            blocks: Vec::new(),
        }
    }

    pub(super) fn push_block(&mut self, block: SlackMessageBodyBlockBuilder) {
        if !block.has_content() {
            return;
        }
        let block_index = self.blocks.len();
        let block = block.finish(slack_message_element_ids(self.id_stem, block_index));
        self.push_aggregate_text(&block.text, &block.links);
        for field in block.fields.iter() {
            self.push_aggregate_text(&field.text, &field.links);
        }
        for element in block.context_elements.iter() {
            if let SlackMessageBodyContextElement::Text(text) = element {
                self.push_aggregate_text(&text.text, &text.links);
            }
        }
        for action in block.actions.iter() {
            let links = action
                .url
                .as_ref()
                .map(|url| SlackMessageBodyLink {
                    range: 0..action.label.len(),
                    target: SlackMessageBodyTarget::Url(url.clone()),
                })
                .into_iter()
                .collect::<Vec<_>>();
            self.push_aggregate_text(&action.label, &links);
        }
        self.blocks.push(block);
    }

    fn push_aggregate_text(&mut self, text: &str, links: &[SlackMessageBodyLink]) {
        if text.is_empty() {
            return;
        }
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        let aggregate_start = self.text.len();
        self.text.push_str(text);
        self.links
            .extend(links.iter().map(|link| SlackMessageBodyLink {
                range: (aggregate_start + link.range.start)..(aggregate_start + link.range.end),
                target: link.target.clone(),
            }));
    }

    pub(super) fn finish(self) -> SlackMessageBody {
        SlackMessageBody {
            text: self.text.into(),
            links: self.links.into(),
            blocks: self.blocks.into(),
        }
    }
}

pub(super) struct SlackMessageBodyBlockBuilder {
    kind: SlackMessageBodyBlockKind,
    text: String,
    styles: Vec<SlackMessageBodyStyleRange>,
    links: Vec<SlackMessageBodyLink>,
    fields: Vec<SlackMessageBodyText>,
    accessory_image: Option<SlackMessageBodyImage>,
    context_elements: Vec<SlackMessageBodyContextElement>,
    actions: Vec<SlackMessageBodyAction>,
}

impl SlackMessageBodyBlockBuilder {
    pub(super) fn new(kind: SlackMessageBodyBlockKind) -> Self {
        Self {
            kind,
            text: String::new(),
            styles: Vec::new(),
            links: Vec::new(),
            fields: Vec::new(),
            accessory_image: None,
            context_elements: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub(super) fn push_field(&mut self, field: SlackMessageBodyBlockBuilder) {
        let field = field.finish_text();
        if !field.text.is_empty() {
            self.fields.push(field);
        }
    }

    pub(super) fn set_accessory_image(&mut self, image: SlackMessageBodyImage) {
        self.accessory_image = Some(image);
    }

    pub(super) fn push_context_text(&mut self, text: SlackMessageBodyBlockBuilder) {
        let text = text.finish_text();
        if !text.text.is_empty() {
            self.context_elements
                .push(SlackMessageBodyContextElement::Text(text));
        }
    }

    pub(super) fn push_context_image(&mut self, image: SlackMessageBodyImage) {
        self.context_elements
            .push(SlackMessageBodyContextElement::Image(image));
    }

    pub(super) fn push_action(&mut self, action: SlackMessageBodyAction) {
        self.actions.push(action);
    }

    pub(super) fn push_decoded_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.push_text(
                &slack_decode_text_entities(text),
                SlackMessageTextStyle::default(),
            );
        }
    }

    pub(super) fn push_text(&mut self, text: &str, style: SlackMessageTextStyle) {
        self.push_target(text, style, None);
    }

    pub(super) fn push_target(
        &mut self,
        text: &str,
        style: SlackMessageTextStyle,
        target: Option<SlackMessageBodyTarget>,
    ) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let range = start..self.text.len();
        if style != SlackMessageTextStyle::default() {
            let extends_previous = self.styles.last().is_some_and(|previous| {
                previous.range.end == range.start && previous.style == style
            });
            if extends_previous {
                self.styles
                    .last_mut()
                    .expect("prepared Slack body style disappeared")
                    .range
                    .end = range.end;
            } else {
                self.styles.push(SlackMessageBodyStyleRange {
                    range: range.clone(),
                    style,
                });
            }
        }
        if let Some(target) = target {
            self.links.push(SlackMessageBodyLink { range, target });
        }
    }

    fn has_content(&self) -> bool {
        !self.text.is_empty()
            || !self.fields.is_empty()
            || self.accessory_image.is_some()
            || !self.context_elements.is_empty()
            || !self.actions.is_empty()
            || self.kind == SlackMessageBodyBlockKind::BlockKitDivider
    }

    fn finish_text(self) -> SlackMessageBodyText {
        let link_ranges = self
            .links
            .iter()
            .map(|link| link.range.clone())
            .collect::<Vec<_>>();
        let code_ranges = self
            .styles
            .iter()
            .filter(|range| range.style.code)
            .map(|range| range.range.clone())
            .collect::<Vec<_>>();
        SlackMessageBodyText {
            text: self.text.into(),
            styles: self.styles.into(),
            links: self.links.into(),
            link_ranges: link_ranges.into(),
            code_ranges: code_ranges.into(),
            code_font_family: "Monaco".into(),
        }
    }

    fn finish(self, element_ids: SlackMessageElementIds) -> SlackMessageBodyBlock {
        let SlackMessageBodyBlockBuilder {
            kind,
            text,
            styles,
            links,
            fields,
            accessory_image,
            context_elements,
            actions,
        } = self;
        let link_ranges = links
            .iter()
            .map(|link| link.range.clone())
            .collect::<Vec<_>>();
        let code_ranges = styles
            .iter()
            .filter(|range| range.style.code)
            .map(|range| range.range.clone())
            .collect::<Vec<_>>();
        SlackMessageBodyBlock {
            kind,
            text: text.into(),
            styles: styles.into(),
            links: links.into(),
            link_ranges: link_ranges.into(),
            code_ranges: code_ranges.into(),
            code_font_family: "Monaco".into(),
            fields: fields.into(),
            accessory_image,
            context_elements: context_elements.into(),
            actions: actions.into(),
            element_ids,
        }
    }
}

pub(in crate::ui::surface::message::rows) fn slack_message_element_ids(
    id_stem: &str,
    block_index: usize,
) -> SlackMessageElementIds {
    SlackMessageElementIds {
        conversation: format!("{id_stem}.conversation.{block_index}").into(),
        thread: format!("{id_stem}.thread.{block_index}").into(),
        all_threads: format!("{id_stem}.all-threads.{block_index}").into(),
        activity: format!("{id_stem}.activity.{block_index}").into(),
        later: format!("{id_stem}.later.{block_index}").into(),
        pins: format!("{id_stem}.pins.{block_index}").into(),
        search: format!("{id_stem}.search.{block_index}").into(),
    }
}

pub(super) fn slack_decode_text_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

pub(super) fn slack_supported_link_url(url: &str) -> bool {
    url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("mailto:")
        || url.starts_with("tel:")
}
