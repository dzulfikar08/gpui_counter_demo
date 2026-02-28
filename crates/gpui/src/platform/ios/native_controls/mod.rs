mod button;
mod checkbox;
mod collection;
mod combo_box;
mod glass_effect_view;
mod icon_button;
mod image_view;
mod menu;
mod outline;
mod popup;
mod progress;
mod search_field;
mod segmented;
mod sidebar;
mod slider;
mod stack_view;
mod stepper;
mod switch;
mod tab_view;
mod table;
mod text_field;
mod tracking_area;
mod visual_effect_view;

pub(crate) use button::*;
pub(crate) use checkbox::*;
pub(crate) use collection::*;
pub(crate) use combo_box::*;
pub(crate) use glass_effect_view::*;
pub(crate) use icon_button::*;
pub(crate) use image_view::*;
pub(crate) use menu::*;
pub(crate) use outline::*;
pub(crate) use popup::*;
pub(crate) use progress::*;
pub(crate) use search_field::*;
pub(crate) use segmented::*;
pub(crate) use sidebar::*;
pub(crate) use slider::*;
pub(crate) use stack_view::*;
pub(crate) use stepper::*;
pub(crate) use switch::*;
pub(crate) use tab_view::*;
pub(crate) use table::*;
pub(crate) use text_field::*;
pub(crate) use tracking_area::*;
pub(crate) use visual_effect_view::*;

use crate::{Bounds, Pixels};
use objc::{msg_send, runtime::Object, sel, sel_impl};
use std::ffi::c_void;

pub(super) const CALLBACK_IVAR: &str = "callbackPtr";

/// The `id` type alias — equivalent to `cocoa::base::id` but available on iOS.
#[allow(non_camel_case_types)]
pub(crate) type id = *mut Object;

/// The nil pointer for ObjC objects.
#[allow(non_upper_case_globals)]
pub(crate) const nil: id = std::ptr::null_mut();

// =============================================================================
// NSString helper
// =============================================================================

/// Creates an autoreleased NSString from a Rust `&str`.
pub(crate) unsafe fn ns_string(string: &str) -> id {
    unsafe {
        let cls = objc::class!(NSString);
        let ns: id = msg_send![cls, alloc];
        let ns: id = msg_send![ns,
            initWithBytes: string.as_ptr()
            length: string.len()
            encoding: 4u64 // NSUTF8StringEncoding
        ];
        let ns: id = msg_send![ns, autorelease];
        ns
    }
}

// =============================================================================
// Shared UIView helpers
// =============================================================================

/// Adds a native UIView as a subview of the given parent view.
pub(crate) unsafe fn attach_native_view_to_parent(view: id, parent: id) {
    unsafe {
        let _: () = msg_send![parent, addSubview: view];
    }
}

/// Positions any UIView within its parent.
/// UIKit uses top-left origin (same as GPUI) — no coordinate flip needed.
pub(crate) unsafe fn set_native_view_frame(
    view: id,
    bounds: Bounds<Pixels>,
    _parent_view: id,
    _scale_factor: f32,
) {
    unsafe {
        let x = bounds.origin.x.0 as f64;
        let y = bounds.origin.y.0 as f64;
        let w = bounds.size.width.0 as f64;
        let h = bounds.size.height.0 as f64;

        // CGRect struct layout: {{x, y}, {w, h}}
        let frame: ((f64, f64), (f64, f64)) = ((x, y), (w, h));
        let _: () = msg_send![view, setFrame: frame];
    }
}

/// Removes a native view from its parent.
pub(crate) unsafe fn remove_native_view_from_parent(view: id) {
    unsafe {
        let _: () = msg_send![view, removeFromSuperview];
    }
}

/// Sets the enabled state of a UIControl (UIButton, UISwitch, UISlider, etc.).
pub(crate) unsafe fn set_native_control_enabled(control: id, enabled: bool) {
    unsafe {
        let _: () = msg_send![control, setEnabled: enabled as i8];
    }
}

/// Sets the alpha (opacity) of a UIView.
pub(crate) unsafe fn set_native_view_alpha(view: id, alpha: f64) {
    unsafe {
        let _: () = msg_send![view, setAlpha: alpha];
    }
}

/// Sets the hidden state of a UIView.
pub(crate) unsafe fn set_native_view_hidden(view: id, hidden: bool) {
    unsafe {
        let _: () = msg_send![view, setHidden: hidden as i8];
    }
}

/// Releases an ObjC object.
pub(crate) unsafe fn release_object(obj: id) {
    if !obj.is_null() {
        let _: () = msg_send![obj, release];
    }
}

/// Releases a target/delegate object and its stored callback.
pub(crate) unsafe fn release_target_with_callback(target: *mut c_void) {
    if !target.is_null() {
        let target = target as id;
        let callback_ptr: *mut c_void = *(*target).get_ivar(CALLBACK_IVAR);
        if !callback_ptr.is_null() {
            // The callback is a Box<Box<dyn Fn(...)>>
            let _ = Box::from_raw(callback_ptr as *mut Box<dyn Fn()>);
        }
        let _: () = msg_send![target, release];
    }
}

// =============================================================================
// UIControl target/action constants
// =============================================================================

/// UIControl.Event.touchUpInside = 1 << 6
pub(super) const UI_CONTROL_EVENT_TOUCH_UP_INSIDE: u64 = 1 << 6;

/// UIControl.Event.valueChanged = 1 << 12
pub(super) const UI_CONTROL_EVENT_VALUE_CHANGED: u64 = 1 << 12;

/// UIControl.Event.editingChanged = 1 << 17
pub(super) const UI_CONTROL_EVENT_EDITING_CHANGED: u64 = 1 << 17;

/// UIControl.Event.editingDidBegin = 1 << 16
pub(super) const UI_CONTROL_EVENT_EDITING_DID_BEGIN: u64 = 1 << 16;

/// UIControl.Event.editingDidEnd = 1 << 18
pub(super) const UI_CONTROL_EVENT_EDITING_DID_END: u64 = 1 << 18;

/// UIControl.Event.editingDidEndOnExit = 1 << 19
pub(super) const UI_CONTROL_EVENT_EDITING_DID_END_ON_EXIT: u64 = 1 << 19;
