//! The `com.canonical.dbusmenu` interface KDE's global menu reads the app
//! menu through.

use std::collections::HashMap;

use zbus::{
    fdo, interface,
    zvariant::{OwnedValue, StructureBuilder, Type, Value},
};

use super::layout::{MenuProperty, MenuTree, ROOT_ID};

/// Where the menu is published on notslack's bus connection.
pub(super) const MENU_OBJECT_PATH: &str = "/MenuBar";

/// The menu never changes after it is published.
const LAYOUT_REVISION: u32 = 1;

pub(super) struct DbusMenu {
    tree: MenuTree,
    activations: async_channel::Sender<i32>,
}

/// An item and, up to the requested depth, its children: `(ia{sv}av)`.
#[derive(serde::Serialize, Type)]
pub(super) struct Layout(i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

impl DbusMenu {
    pub(super) fn new(tree: MenuTree, activations: async_channel::Sender<i32>) -> Self {
        Self { tree, activations }
    }

    fn properties(&self, id: i32, names: &[String]) -> HashMap<String, OwnedValue> {
        self.tree
            .properties(id, names)
            .into_iter()
            .map(|(name, property)| (name.to_string(), property_value(property)))
            .collect()
    }

    fn layout(&self, id: i32, depth: i32, names: &[String]) -> Layout {
        let children = if depth == 0 {
            Vec::new()
        } else {
            self.tree
                .children(id)
                .iter()
                .map(|child| layout_value(self.layout(*child, depth.saturating_sub(1), names)))
                .collect()
        };
        Layout(id, self.properties(id, names), children)
    }

    fn activate(&self, id: i32, event_id: &str) -> bool {
        if !self.tree.contains(id) {
            return false;
        }
        if event_id == "clicked" {
            // Unbounded, so this fails only once notslack is shutting down.
            let _ = self.activations.try_send(id);
        }
        true
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl DbusMenu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> fdo::Result<(u32, Layout)> {
        if !self.tree.contains(parent_id) {
            return Err(unknown_item(parent_id));
        }
        Ok((
            LAYOUT_REVISION,
            self.layout(parent_id, recursion_depth, &property_names),
        ))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
        let ids = if ids.is_empty() { self.all_ids() } else { ids };
        ids.into_iter()
            .filter(|id| self.tree.contains(*id))
            .map(|id| (id, self.properties(id, &property_names)))
            .collect()
    }

    fn get_property(&self, id: i32, name: String) -> fdo::Result<OwnedValue> {
        self.properties(id, std::slice::from_ref(&name))
            .remove(&name)
            .ok_or_else(|| fdo::Error::InvalidArgs(format!("item {id} has no property {name}")))
    }

    fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        self.activate(id, &event_id);
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        events
            .into_iter()
            .filter(|(id, event_id, _, _)| !self.activate(*id, event_id))
            .map(|(id, _, _, _)| id)
            .collect()
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        let unknown = ids
            .into_iter()
            .filter(|id| !self.tree.contains(*id))
            .collect();
        (Vec::new(), unknown)
    }
}

impl DbusMenu {
    fn all_ids(&self) -> Vec<i32> {
        let mut ids = vec![ROOT_ID];
        let mut index = 0;
        while let Some(id) = ids.get(index).copied() {
            ids.extend_from_slice(self.tree.children(id));
            index += 1;
        }
        ids
    }
}

fn unknown_item(id: i32) -> fdo::Error {
    fdo::Error::InvalidArgs(format!("no menu item {id}"))
}

fn property_value(property: MenuProperty) -> OwnedValue {
    let value = match property {
        MenuProperty::Text(text) => Value::from(text),
        MenuProperty::Label(label) => Value::from(label),
        MenuProperty::Flag(flag) => Value::from(flag),
        MenuProperty::Number(number) => Value::from(number),
        MenuProperty::Shortcut(shortcut) => Value::from(shortcut),
    };
    OwnedValue::try_from(value).expect("menu properties hold no file descriptors")
}

/// A child layout, which dbusmenu nests as a variant.
fn layout_value(Layout(id, properties, children): Layout) -> OwnedValue {
    let structure = StructureBuilder::new()
        .add_field(id)
        .add_field(properties)
        .add_field(children)
        .build()
        .expect("a layout has three fields");
    OwnedValue::try_from(Value::from(structure)).expect("layouts hold no file descriptors")
}

#[cfg(test)]
mod tests {
    use gpui::{actions, Menu, MenuItem};

    use super::*;
    use crate::global_menu::layout::build_menu;

    actions!(dbusmenu_test, [Quit]);

    fn menu() -> (DbusMenu, async_channel::Receiver<i32>) {
        let menus = [Menu::new("notslack")
            .items([MenuItem::action("Quit notslack", Quit)])
            .owned()];
        let (sender, receiver) = async_channel::unbounded();
        (
            DbusMenu::new(build_menu(&menus, |_| Vec::new()).tree, sender),
            receiver,
        )
    }

    #[test]
    fn the_whole_layout_nests_children_as_variants() {
        let (menu, _) = menu();
        let Layout(id, properties, children) = menu.layout(ROOT_ID, -1, &[]);
        assert_eq!(id, ROOT_ID);
        assert!(properties.contains_key("children-display"));
        assert_eq!(children.len(), 1);
        let signature = Value::from(children[0].try_clone().unwrap())
            .value_signature()
            .to_string();
        assert_eq!(signature, "(ia{sv}av)");
    }

    #[test]
    fn a_depth_of_zero_leaves_out_the_children() {
        let (menu, _) = menu();
        assert!(menu.layout(ROOT_ID, 0, &[]).2.is_empty());
    }

    #[test]
    fn clicking_an_item_reports_its_id_and_other_events_do_not() {
        let (menu, activations) = menu();
        assert!(menu.activate(2, "hovered"));
        assert!(activations.try_recv().is_err());
        assert!(menu.activate(2, "clicked"));
        assert_eq!(activations.try_recv(), Ok(2));
        assert!(!menu.activate(42, "clicked"));
    }

    #[test]
    fn every_item_is_listed_when_no_ids_are_asked_for() {
        let (menu, _) = menu();
        assert_eq!(menu.all_ids(), [0, 1, 2]);
    }
}
