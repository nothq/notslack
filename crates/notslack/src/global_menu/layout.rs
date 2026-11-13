//! The app menu as a tree of numbered dbusmenu items.

use std::collections::HashMap;

use gpui::{Action, KeyBinding, OwnedMenu, OwnedMenuItem};

/// The invisible item every menu bar hangs from.
pub(super) const ROOT_ID: i32 = 0;

#[derive(Clone, Debug, PartialEq, Eq)]
enum MenuEntry {
    Root,
    Submenu {
        label: String,
        enabled: bool,
    },
    Item {
        label: String,
        enabled: bool,
        checked: bool,
        shortcut: Vec<Vec<String>>,
    },
    Separator,
}

#[derive(Debug)]
struct MenuNode {
    entry: MenuEntry,
    children: Vec<i32>,
}

/// A dbusmenu property value, before it is put on the bus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum MenuProperty {
    Text(&'static str),
    Label(String),
    Flag(bool),
    Number(i32),
    Shortcut(Vec<Vec<String>>),
}

/// Every menu item, indexed by its dbusmenu id.
#[derive(Debug)]
pub(super) struct MenuTree {
    nodes: Vec<MenuNode>,
}

/// The menu tree and the action behind each clickable item.
pub(super) struct BuiltMenu {
    pub(super) tree: MenuTree,
    pub(super) actions: HashMap<i32, Box<dyn Action>>,
}

/// Numbers `menus` depth first under the root. `shortcut` names the key
/// binding shown beside an action.
pub(super) fn build_menu(
    menus: &[OwnedMenu],
    shortcut: impl Fn(&dyn Action) -> Vec<Vec<String>>,
) -> BuiltMenu {
    let mut built = BuiltMenu {
        tree: MenuTree {
            nodes: vec![MenuNode {
                entry: MenuEntry::Root,
                children: Vec::new(),
            }],
        },
        actions: HashMap::new(),
    };
    for menu in menus {
        built.add_menu(ROOT_ID, menu, &shortcut);
    }
    built
}

impl BuiltMenu {
    fn add_menu(
        &mut self,
        parent: i32,
        menu: &OwnedMenu,
        shortcut: &impl Fn(&dyn Action) -> Vec<Vec<String>>,
    ) {
        let menu_id = self.tree.push(
            parent,
            MenuEntry::Submenu {
                label: menu.name.to_string(),
                enabled: !menu.disabled,
            },
        );
        for item in &menu.items {
            match item {
                OwnedMenuItem::Separator => {
                    self.tree.push(menu_id, MenuEntry::Separator);
                }
                OwnedMenuItem::Submenu(submenu) => self.add_menu(menu_id, submenu, shortcut),
                // The system menus (such as Services) exist only on macOS.
                OwnedMenuItem::SystemMenu(_) => {}
                OwnedMenuItem::Action {
                    name,
                    action,
                    checked,
                    disabled,
                    ..
                } => {
                    let item_id = self.tree.push(
                        menu_id,
                        MenuEntry::Item {
                            label: name.clone(),
                            enabled: !disabled,
                            checked: *checked,
                            shortcut: shortcut(action.as_ref()),
                        },
                    );
                    self.actions.insert(item_id, action.boxed_clone());
                }
            }
        }
    }
}

impl MenuTree {
    fn push(&mut self, parent: i32, entry: MenuEntry) -> i32 {
        let id = i32::try_from(self.nodes.len()).expect("menu has fewer than 2^31 items");
        self.nodes.push(MenuNode {
            entry,
            children: Vec::new(),
        });
        self.nodes[parent as usize].children.push(id);
        id
    }

    fn node(&self, id: i32) -> Option<&MenuNode> {
        usize::try_from(id)
            .ok()
            .and_then(|index| self.nodes.get(index))
    }

    pub(super) fn contains(&self, id: i32) -> bool {
        self.node(id).is_some()
    }

    pub(super) fn children(&self, id: i32) -> &[i32] {
        self.node(id).map_or(&[], |node| &node.children)
    }

    /// The item's properties, only those in `names` unless it is empty.
    pub(super) fn properties(
        &self,
        id: i32,
        names: &[String],
    ) -> Vec<(&'static str, MenuProperty)> {
        let Some(node) = self.node(id) else {
            return Vec::new();
        };
        let mut properties = Vec::new();
        match &node.entry {
            MenuEntry::Root => {
                properties.push(("children-display", MenuProperty::Text("submenu")));
            }
            MenuEntry::Submenu { label, enabled } => {
                properties.push(("label", MenuProperty::Label(dbusmenu_label(label))));
                properties.push(("enabled", MenuProperty::Flag(*enabled)));
                properties.push(("children-display", MenuProperty::Text("submenu")));
            }
            MenuEntry::Item {
                label,
                enabled,
                checked,
                shortcut,
            } => {
                properties.push(("label", MenuProperty::Label(dbusmenu_label(label))));
                properties.push(("enabled", MenuProperty::Flag(*enabled)));
                if *checked {
                    properties.push(("toggle-type", MenuProperty::Text("checkmark")));
                    properties.push(("toggle-state", MenuProperty::Number(1)));
                }
                if !shortcut.is_empty() {
                    properties.push(("shortcut", MenuProperty::Shortcut(shortcut.clone())));
                }
            }
            MenuEntry::Separator => {
                properties.push(("type", MenuProperty::Text("separator")));
            }
        }
        if !names.is_empty() {
            properties.retain(|(name, _)| names.iter().any(|wanted| wanted == name));
        }
        properties
    }
}

/// dbusmenu reads an underscore as the mnemonic marker, so a literal one is
/// doubled.
fn dbusmenu_label(label: &str) -> String {
    label.replace('_', "__")
}

/// A key binding as dbusmenu spells it: one list of key names per keystroke.
pub(super) fn dbusmenu_shortcut(binding: &KeyBinding) -> Vec<Vec<String>> {
    binding
        .keystrokes()
        .iter()
        .map(|keystroke| {
            let modifiers = keystroke.modifiers();
            [
                (modifiers.control, "Control"),
                (modifiers.alt, "Alt"),
                (modifiers.shift, "Shift"),
                (modifiers.platform, "Super"),
            ]
            .into_iter()
            .filter(|(held, _)| *held)
            .map(|(_, name)| name.to_string())
            .chain(std::iter::once(dbusmenu_key(keystroke.key())))
            .collect()
        })
        .collect()
}

fn dbusmenu_key(key: &str) -> String {
    let mut chars = key.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use gpui::{actions, Menu, MenuItem};

    use super::*;

    actions!(global_menu_test, [Quit, Preferences]);

    fn menus() -> Vec<OwnedMenu> {
        vec![Menu::new("notslack")
            .items([
                MenuItem::action("Preferences_", Preferences),
                MenuItem::separator(),
                MenuItem::action("Quit notslack", Quit),
            ])
            .owned()]
    }

    fn no_shortcut(_: &dyn Action) -> Vec<Vec<String>> {
        Vec::new()
    }

    #[test]
    fn menus_are_numbered_depth_first_under_the_root() {
        let built = build_menu(&menus(), no_shortcut);
        assert_eq!(built.tree.children(ROOT_ID), [1]);
        assert_eq!(built.tree.children(1), [2, 3, 4]);
        assert_eq!(
            built.tree.properties(1, &[]),
            [
                ("label", MenuProperty::Label("notslack".to_string())),
                ("enabled", MenuProperty::Flag(true)),
                ("children-display", MenuProperty::Text("submenu")),
            ]
        );
        assert_eq!(
            built.tree.properties(3, &[]),
            [("type", MenuProperty::Text("separator"))]
        );
    }

    #[test]
    fn each_action_item_keeps_its_action() {
        let built = build_menu(&menus(), no_shortcut);
        let mut ids = built.actions.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, [2, 4]);
        assert!(built.actions[&4].partial_eq(&Quit));
    }

    #[test]
    fn labels_escape_underscores_and_requests_filter_properties() {
        let built = build_menu(&menus(), |_| {
            vec![vec!["Control".to_string(), "Q".to_string()]]
        });
        assert_eq!(
            built.tree.properties(2, &["label".to_string()]),
            [("label", MenuProperty::Label("Preferences__".to_string()))]
        );
        assert_eq!(
            built.tree.properties(4, &["shortcut".to_string()]),
            [(
                "shortcut",
                MenuProperty::Shortcut(vec![vec!["Control".to_string(), "Q".to_string()]])
            )]
        );
    }

    #[test]
    fn unknown_ids_have_no_children_or_properties() {
        let built = build_menu(&menus(), no_shortcut);
        assert!(!built.tree.contains(99));
        assert!(built.tree.children(-1).is_empty());
        assert!(built.tree.properties(99, &[]).is_empty());
    }

    #[test]
    fn shortcuts_name_modifiers_then_the_key() {
        let binding = KeyBinding::new("ctrl-shift-q", Quit, None);
        assert_eq!(
            dbusmenu_shortcut(&binding),
            [["Control", "Shift", "Q"].map(String::from).to_vec()]
        );
    }
}
