use super::{id, ns_string, CALLBACK_IVAR, UI_CONTROL_EVENT_TOUCH_UP_INSIDE};
use ctor::ctor;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel},
    sel, sel_impl,
};
use std::{ffi::c_void, ptr};

static mut CHECKBOX_TARGET_CLASS: *const Class = ptr::null();

#[ctor]
unsafe fn build_checkbox_target_class() {
    unsafe {
        let mut decl = ClassDecl::new("GPUIiOSNativeCheckboxTarget", class!(NSObject)).unwrap();
        decl.add_ivar::<*mut c_void>(CALLBACK_IVAR);
        decl.add_method(
            sel!(checkboxAction:),
            checkbox_action as extern "C" fn(&Object, Sel, id),
        );
        CHECKBOX_TARGET_CLASS = decl.register();
    }
}

extern "C" fn checkbox_action(this: &Object, _sel: Sel, sender: id) {
    unsafe {
        let ptr: *mut c_void = *this.get_ivar(CALLBACK_IVAR);
        if !ptr.is_null() {
            // Toggle: read current selected state
            let selected: bool = msg_send![sender, isSelected];
            let callback = &*(ptr as *const Box<dyn Fn(bool)>);
            callback(!selected);
        }
    }
}

/// Creates a native checkbox using UIButton with SF Symbol checkmark.
/// iOS has no built-in checkbox — we use a UIButton that toggles between
/// "square" and "checkmark.square.fill" SF Symbols.
pub(crate) unsafe fn create_native_checkbox(title: &str) -> id {
    unsafe {
        let button: id = msg_send![class!(UIButton), buttonWithType: 1i64]; // UIButtonTypeSystem
        let _: () = msg_send![button, retain];
        let _: () = msg_send![button, setTitle: ns_string(title) forState: 0u64];

        // Set unchecked image
        let unchecked: id = msg_send![class!(UIImage), systemImageNamed: ns_string("square")];
        let _: () = msg_send![button, setImage: unchecked forState: 0u64];

        button
    }
}

/// Sets the checkbox title.
pub(crate) unsafe fn set_native_checkbox_title(checkbox: id, title: &str) {
    unsafe {
        let _: () = msg_send![checkbox, setTitle: ns_string(title) forState: 0u64];
    }
}

/// Sets the checked state (updates the SF Symbol icon).
pub(crate) unsafe fn set_native_checkbox_state(checkbox: id, checked: bool) {
    unsafe {
        let _: () = msg_send![checkbox, setSelected: checked as i8];
        let symbol = if checked {
            "checkmark.square.fill"
        } else {
            "square"
        };
        let image: id = msg_send![class!(UIImage), systemImageNamed: ns_string(symbol)];
        let _: () = msg_send![checkbox, setImage: image forState: 0u64];
    }
}

/// Sets target/action for the checkbox toggle.
pub(crate) unsafe fn set_native_checkbox_action(
    checkbox: id,
    callback: Box<dyn Fn(bool)>,
) -> *mut c_void {
    unsafe {
        let target: id = msg_send![CHECKBOX_TARGET_CLASS, alloc];
        let target: id = msg_send![target, init];

        let callback_ptr = Box::into_raw(Box::new(callback)) as *mut c_void;
        (*target).set_ivar::<*mut c_void>(CALLBACK_IVAR, callback_ptr);

        let _: () = msg_send![checkbox,
            addTarget: target
            action: sel!(checkboxAction:)
            forControlEvents: UI_CONTROL_EVENT_TOUCH_UP_INSIDE
        ];

        target as *mut c_void
    }
}

/// Releases the checkbox target and callback.
pub(crate) unsafe fn release_native_checkbox_target(target: *mut c_void) {
    unsafe {
        if !target.is_null() {
            let target = target as id;
            let callback_ptr: *mut c_void = *(*target).get_ivar(CALLBACK_IVAR);
            if !callback_ptr.is_null() {
                let _ = Box::from_raw(callback_ptr as *mut Box<dyn Fn(bool)>);
            }
            let _: () = msg_send![target, release];
        }
    }
}

/// Releases the checkbox (UIButton).
pub(crate) unsafe fn release_native_checkbox(checkbox: id) {
    unsafe {
        if !checkbox.is_null() {
            let _: () = msg_send![checkbox, release];
        }
    }
}
