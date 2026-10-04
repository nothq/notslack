use super::{svg_from_body, Arc, Image, SlackShellIcon};

pub(super) fn slack_reaction_picker_icon_image(
    icon: SlackShellIcon,
    fill: u32,
) -> Option<Arc<Image>> {
    let body = match icon {
        SlackShellIcon::ReactionCategorySmileys => {
            r#"<circle cx="10" cy="10" r="7"/><path d="M7.5 8h.01M12.5 8h.01M6.6 11.5c.8 2 2 3 3.4 3s2.6-1 3.4-3"/>"#
        }
        SlackShellIcon::ReactionCategoryAnimals => {
            r#"<path d="M16.8 3.2C10.3 3.6 5.6 6.5 4.2 11.5c-.5 1.8.4 3.5 2.2 4 4.8 1.4 8.1-3.1 10.4-12.3Z"/><path d="M4.3 16.7c1.5-3.6 4.1-6.4 8-8.4"/>"#
        }
        SlackShellIcon::ReactionCategoryFood => {
            r#"<path d="M4 9.2h12M3.2 12h13.6M4 14.8h12M4 9.2C4.4 5.8 6.5 4 10 4s5.6 1.8 6 5.2M4 14.8v.7c0 .8.7 1.5 1.5 1.5h9c.8 0 1.5-.7 1.5-1.5v-.7M8 7.2h.01M12 6.5h.01"/>"#
        }
        SlackShellIcon::ReactionCategoryTravel => {
            r#"<path d="m3 11.1 5.8.9 2.8 5 1.4-.5-1.2-5.2 4.2-2.1c1.3-.7 1.8-1.8 1.4-2.5-.4-.8-1.7-.8-3 0l-4.1 2.2-3.9-3.6-1.2.7 2.7 4.5-3.6 1.9Z"/>"#
        }
        SlackShellIcon::ReactionCategoryActivities => {
            r#"<path d="M4.1 15.9c-2.6-2.6-.6-7.4 2.4-10.4s7.8-5 10.4-2.4.6 7.4-2.4 10.4-7.8 5-10.4 2.4Z"/><path d="m6.4 13.6 7.2-7.2M8.2 9.2l2.6 2.6M9.8 7.6l2.6 2.6"/>"#
        }
        SlackShellIcon::ReactionCategoryObjects => {
            r#"<path d="M14.8 8.3c0-2.8-2.1-5.1-4.8-5.1S5.2 5.5 5.2 8.3c0 1.8.9 3.3 2.2 4.2v1.7h5.2v-1.7c1.3-.9 2.2-2.4 2.2-4.2ZM7.6 16.8h4.8M7.4 14.2h5.2"/>"#
        }
        SlackShellIcon::ReactionCategorySymbols => {
            r#"<circle cx="10" cy="10" r="7"/><path d="M10 3v14M10 10 5.2 14M10 10l4.8 4"/>"#
        }
        SlackShellIcon::ReactionCategoryFlags => {
            r#"<path d="M5 18V3.2M5 4c3.5-2 6 2 10-.2v8.5c-4 2.2-6.5-1.8-10 .2"/>"#
        }
        SlackShellIcon::ReactionCategoryCustom => {
            r#"<path d="M10 3.2a6.8 6.8 0 1 0 0 13.6 6.8 6.8 0 0 0 0-13.6Z"/><path d="M7.4 8h.01M12.6 8h.01M6.8 11.5c.9 1.5 2 2.3 3.2 2.3s2.3-.8 3.2-2.3M15.8 4.2l1-1M16.8 6.2h1.4"/>"#
        }
        _ => return None,
    };
    Some(svg_from_body(
        "0 0 20 20",
        format!(
            r##"<g fill="none" stroke="#{fill:06x}" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.6">{body}</g>"##
        ),
    ))
}
