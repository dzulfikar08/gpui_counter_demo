// iOS has no direct NSOutlineView equivalent.
// We implement a basic expandable list using UITableView.
// For a full implementation, a UITableView with expandable sections
// or a custom tree data source would be needed.

use super::{id, ns_string};
use objc::{class, msg_send, sel, sel_impl};
use std::ffi::c_void;

/// Creates a UITableView configured for outline-style display.
pub(crate) unsafe fn create_native_outline_view() -> id {
    unsafe {
        // UITableViewStyleGrouped = 1 — gives section headers for tree-like structure
        let table: id = msg_send![class!(UITableView), alloc];
        let table: id = msg_send![table, initWithFrame:
            ((0.0f64, 0.0f64), (320.0f64, 480.0f64))
            style: 1i64
        ];
        table
    }
}

/// Sets the outline items. Simplified — flattens the tree for now.
pub(crate) unsafe fn set_native_outline_items(
    _table: id,
    _items: *mut c_void,
) -> *mut c_void {
    // Full implementation would set up a tree data source.
    std::ptr::null_mut()
}

/// Syncs column width. No-op on iOS.
pub(crate) unsafe fn sync_native_outline_column_width(_table: id) {}

/// Sets highlight style. No-op on iOS.
pub(crate) unsafe fn set_native_outline_highlight_style(_table: id, _style: i64) {}

/// Sets row height.
pub(crate) unsafe fn set_native_outline_row_height(table: id, row_height: f64) {
    unsafe {
        let _: () = msg_send![table, setRowHeight: row_height];
    }
}

/// Releases the outline data source.
pub(crate) unsafe fn release_native_outline_target(_target: *mut c_void) {}

/// Releases the outline view (UITableView).
pub(crate) unsafe fn release_native_outline_view(table: id) {
    unsafe {
        if !table.is_null() {
            let _: () = msg_send![table, release];
        }
    }
}
