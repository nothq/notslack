//! Telling the desktop which bus object holds a window's menu: KWin's appmenu
//! protocol on Wayland, the AppMenu registrar on X11.

use std::ffi::c_void;

use gpui::Window;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_client::{
    backend::{Backend, ObjectId},
    globals::{registry_queue_init, BindError, GlobalListContents},
    protocol::{wl_registry::WlRegistry, wl_surface::WlSurface},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols_plasma::appmenu::client::{
    org_kde_kwin_appmenu::OrgKdeKwinAppmenu, org_kde_kwin_appmenu_manager::OrgKdeKwinAppmenuManager,
};

use super::dbusmenu::MENU_OBJECT_PATH;

const REGISTRAR_SERVICE: &str = "com.canonical.AppMenu.Registrar";
const REGISTRAR_PATH: &str = "/com/canonical/AppMenu/Registrar";

/// The window's link to its menu, kept for as long as the menu is shown.
pub(super) enum MenuRegistration {
    Wayland {
        _connection: Connection,
        appmenu: OrgKdeKwinAppmenu,
    },
    X11,
}

impl Drop for MenuRegistration {
    fn drop(&mut self) {
        if let Self::Wayland { appmenu, .. } = self {
            appmenu.release();
        }
    }
}

/// Points the desktop at the menu `bus` serves for `window`. `Ok(None)` when
/// the desktop has no global menu.
pub(super) fn register_window_menu(
    window: &Window,
    bus: &zbus::blocking::Connection,
) -> Result<Option<MenuRegistration>, String> {
    let service = bus
        .unique_name()
        .ok_or("the session bus gave notslack no name")?
        .to_string();
    let window_handle =
        HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
    match window_handle.as_raw() {
        RawWindowHandle::Wayland(surface) => {
            let display = match HasDisplayHandle::display_handle(window)
                .map_err(|error| error.to_string())?
                .as_raw()
            {
                RawDisplayHandle::Wayland(display) => display.display,
                _ => return Err("a Wayland window without a Wayland display".to_string()),
            };
            // SAFETY: both pointers come from GPUI's live connection and
            // window, which outlive the menu.
            unsafe { register_wayland(display.as_ptr(), surface.surface.as_ptr(), &service) }
        }
        RawWindowHandle::Xcb(xcb) => register_x11(bus, xcb.window.get()),
        RawWindowHandle::Xlib(xlib) => {
            let window = u32::try_from(xlib.window).map_err(|error| error.to_string())?;
            register_x11(bus, window)
        }
        _ => Ok(None),
    }
}

struct AppmenuState;

impl Dispatch<WlRegistry, GlobalListContents> for AppmenuState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

wayland_client::delegate_noop!(AppmenuState: ignore OrgKdeKwinAppmenuManager);
wayland_client::delegate_noop!(AppmenuState: ignore OrgKdeKwinAppmenu);

/// Uses GPUI's Wayland connection through an event queue of its own, so
/// GPUI's events are left alone.
unsafe fn register_wayland(
    display: *mut c_void,
    surface: *mut c_void,
    service: &str,
) -> Result<Option<MenuRegistration>, String> {
    let backend = Backend::from_foreign_display(display.cast());
    let connection = Connection::from_backend(backend);
    let (globals, mut queue) =
        registry_queue_init::<AppmenuState>(&connection).map_err(|error| error.to_string())?;
    let handle = queue.handle();
    let manager: OrgKdeKwinAppmenuManager = match globals.bind(&handle, 1..=2, ()) {
        Ok(manager) => manager,
        Err(BindError::NotPresent) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let surface_id = ObjectId::from_ptr(WlSurface::interface(), surface.cast())
        .map_err(|error| error.to_string())?;
    let surface = WlSurface::from_id(&connection, surface_id).map_err(|error| error.to_string())?;
    let appmenu = manager.create(&surface, &handle, ());
    appmenu.set_address(service.to_string(), MENU_OBJECT_PATH.to_string());
    if manager.version() >= 2 {
        manager.release();
    }
    queue
        .roundtrip(&mut AppmenuState)
        .map_err(|error| error.to_string())?;
    Ok(Some(MenuRegistration::Wayland {
        _connection: connection,
        appmenu,
    }))
}

fn register_x11(
    bus: &zbus::blocking::Connection,
    window: u32,
) -> Result<Option<MenuRegistration>, String> {
    let path = zbus::zvariant::ObjectPath::try_from(MENU_OBJECT_PATH)
        .map_err(|error| error.to_string())?;
    match bus.call_method(
        Some(REGISTRAR_SERVICE),
        REGISTRAR_PATH,
        Some(REGISTRAR_SERVICE),
        "RegisterWindow",
        &(window, path),
    ) {
        Ok(_) => Ok(Some(MenuRegistration::X11)),
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" =>
        {
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}
