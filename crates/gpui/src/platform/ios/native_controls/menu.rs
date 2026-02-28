// iOS menu button implementation using UIButton + UIMenu (iOS 14+).
// NSMenu doesn't exist on iOS. UIMenu is the equivalent.

use super::{id, nil, ns_string, CALLBACK_IVAR, UI_CONTROL_EVENT_TOUCH_UP_INSIDE};
use ctor::ctor;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use std::{ffi::c_void, ptr};

/// Creates a UIButton configured to show a menu on tap.
pub(crate) unsafe fn create_native_menu_button(title: &str) -> id {
    unsafe {
        let button: id = msg_send![class!(UIButton), buttonWithType: 1i64];
        let _: () = msg_send![button, retain];
        let _: () = msg_send![button, setTitle: ns_string(title) forState: 0u64];
        let _: () = msg_send![button, setShowsMenuAsPrimaryAction: true as i8];
        button
    }
}

/// Creates a UIButton with a context menu (same as menu button on iOS).
pub(crate) unsafe fn create_native_context_menu_button(title: &str) -> id {
    unsafe {
        create_native_menu_button(title)
    }
}

/// Sets the menu button title.
pub(crate) unsafe fn set_native_menu_button_title(button: id, title: &str) {
    unsafe {
        let _: () = msg_send![button, setTitle: ns_string(title) forState: 0u64];
    }
}

/// Sets the menu items using UIMenu + UIAction.
pub(crate) unsafe fn set_native_menu_button_items(
    button: id,
    items: &[&str],
) {
    unsafe {
        let mut actions: Vec<id> = Vec::with_capacity(items.len());
        for item in items {
            let action: id = msg_send![class!(UIAction),
                actionWithTitle: ns_string(item)
                image: nil
                identifier: nil
                handler: ptr::null::<c_void>()
            ];
            actions.push(action);
        }

        let array: id = msg_send![class!(NSArray),
            arrayWithObjects: actions.as_ptr()
            count: actions.len()
        ];
        let menu: id = msg_send![class!(UIMenu),
            menuWithTitle: ns_string("")
            children: array
        ];
        let _: () = msg_send![button, setMenu: menu];
    }
}

/// Creates a menu target for callbacks.
pub(crate) unsafe fn create_native_menu_target(
    _callback: Box<dyn Fn(usize)>,
) -> *mut c_void {
    // UIMenu uses block-based handlers, not target/action.
    // The element layer should rebuild menus with proper action closures.
    std::ptr::null_mut()
}

/// Releases the menu target.
pub(crate) unsafe fn release_native_menu_button_target(_target: *mut c_void) {}

/// Releases a menu button.
pub(crate) unsafe fn release_native_menu_button(button: id) {
    unsafe {
        if !button.is_null() {
            let _: () = msg_send![button, release];
        }
    }
}

/// Shows a popup menu. On iOS, menus are shown automatically via UIButton.showsMenuAsPrimaryAction.
pub(crate) unsafe fn show_popup_menu_deferred(
    _button: id,
    _menu: id,
    _location: (f64, f64),
) {}
