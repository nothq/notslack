//! The app menu in KDE Plasma's global menu, as macOS shows it in the menu
//! bar. Desktops without a global menu are left as they are.

mod dbusmenu;
mod layout;
mod window;

use gpui::{App, Task, Window};

use dbusmenu::{DbusMenu, MENU_OBJECT_PATH};
use layout::{build_menu, dbusmenu_shortcut};
use window::{register_window_menu, MenuRegistration};

/// Keeps the menu published; dropping it takes the menu away.
pub struct GlobalMenu {
    _registration: MenuRegistration,
    _bus: zbus::blocking::Connection,
    _activations: Task<()>,
}

/// Publishes the menus set with `cx.set_menus` as `window`'s global menu.
pub fn export_global_menu(window: &Window, cx: &mut App) -> Option<GlobalMenu> {
    let menus = cx.get_menus().filter(|menus| !menus.is_empty())?;
    let built = build_menu(&menus, |action| {
        window
            .highest_precedence_binding_for_action(action)
            .map(|binding| dbusmenu_shortcut(&binding))
            .unwrap_or_default()
    });
    let (sender, receiver) = async_channel::unbounded();
    let published = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.serve_at(MENU_OBJECT_PATH, DbusMenu::new(built.tree, sender)))
        .and_then(zbus::blocking::connection::Builder::build)
        .map_err(|error| error.to_string())
        .and_then(|bus| Ok((register_window_menu(window, &bus)?, bus)));
    let (registration, bus) = match published {
        Ok((Some(registration), bus)) => (registration, bus),
        Ok((None, _)) => return None,
        Err(error) => {
            eprintln!("notslack could not publish its global menu: {error}");
            return None;
        }
    };
    let actions = built.actions;
    let activations = cx.spawn(async move |cx| {
        while let Ok(id) = receiver.recv().await {
            if let Some(action) = actions.get(&id) {
                cx.update(|cx| cx.dispatch_action(action.as_ref()));
            }
        }
    });
    Some(GlobalMenu {
        _registration: registration,
        _bus: bus,
        _activations: activations,
    })
}
