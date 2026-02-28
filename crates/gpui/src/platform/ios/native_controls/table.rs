// UITableView wrapper for iOS.
// This is a simplified implementation that creates a basic UITableView
// with a data source pattern matching the macOS NSTableView wrapper.

use super::{id, nil, ns_string, CALLBACK_IVAR};
use ctor::ctor;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    runtime::{Class, Object, Sel, BOOL, YES, NO},
    sel, sel_impl,
};
use std::{ffi::c_void, ptr};

/// Row data for the table.
pub(crate) struct IosTableRow {
    pub text: String,
}

const DATA_IVAR: &str = "dataPtr";

static mut TABLE_DATA_SOURCE_CLASS: *const Class = ptr::null();

#[ctor]
unsafe fn build_table_data_source_class() {
    unsafe {
        let mut decl = ClassDecl::new("GPUIiOSTableDataSource", class!(NSObject)).unwrap();
        decl.add_ivar::<*mut c_void>(DATA_IVAR);
        decl.add_ivar::<*mut c_void>(CALLBACK_IVAR);

        // UITableViewDataSource
        decl.add_method(
            sel!(tableView:numberOfRowsInSection:),
            number_of_rows as extern "C" fn(&Object, Sel, id, isize) -> isize,
        );
        decl.add_method(
            sel!(tableView:cellForRowAtIndexPath:),
            cell_for_row as extern "C" fn(&Object, Sel, id, id) -> id,
        );

        TABLE_DATA_SOURCE_CLASS = decl.register();
    }
}

extern "C" fn number_of_rows(this: &Object, _sel: Sel, _table: id, _section: isize) -> isize {
    unsafe {
        let ptr: *mut c_void = *this.get_ivar(DATA_IVAR);
        if ptr.is_null() {
            return 0;
        }
        let rows = &*(ptr as *const Vec<IosTableRow>);
        rows.len() as isize
    }
}

extern "C" fn cell_for_row(this: &Object, _sel: Sel, table: id, index_path: id) -> id {
    unsafe {
        let row_index: isize = msg_send![index_path, row];
        let cell_id = ns_string("GPUICell");
        let mut cell: id = msg_send![table,
            dequeueReusableCellWithIdentifier: cell_id
        ];
        if cell == nil {
            // UITableViewCellStyleDefault = 0
            cell = msg_send![class!(UITableViewCell), alloc];
            cell = msg_send![cell, initWithStyle: 0i64 reuseIdentifier: cell_id];
        }

        let ptr: *mut c_void = *this.get_ivar(DATA_IVAR);
        if !ptr.is_null() {
            let rows = &*(ptr as *const Vec<IosTableRow>);
            if let Some(row) = rows.get(row_index as usize) {
                let text_label: id = msg_send![cell, textLabel];
                let _: () = msg_send![text_label, setText: ns_string(&row.text)];
            }
        }

        cell
    }
}

/// Creates a UITableView wrapped in its parent structure.
pub(crate) unsafe fn create_native_table_view() -> id {
    unsafe {
        // UITableViewStylePlain = 0
        let table: id = msg_send![class!(UITableView), alloc];
        let table: id = msg_send![table, initWithFrame:
            ((0.0f64, 0.0f64), (320.0f64, 480.0f64))
            style: 0i64
        ];
        table
    }
}

/// Sets table column title. No-op on iOS (UITableView uses section headers).
pub(crate) unsafe fn set_native_table_column_title(_scroll_view: id, _title: &str) {}

/// Sets table column width. No-op on iOS.
pub(crate) unsafe fn set_native_table_column_width(_scroll_view: id, _width: f64) {}

/// Sets table items by creating a data source.
pub(crate) unsafe fn set_native_table_items(
    table: id,
    items: Vec<IosTableRow>,
) -> *mut c_void {
    unsafe {
        let source: id = msg_send![TABLE_DATA_SOURCE_CLASS, alloc];
        let source: id = msg_send![source, init];

        let data = Box::into_raw(Box::new(items)) as *mut c_void;
        (*source).set_ivar::<*mut c_void>(DATA_IVAR, data);

        let _: () = msg_send![table, setDataSource: source];
        let _: () = msg_send![table, reloadData];

        source as *mut c_void
    }
}

/// Sets row height.
pub(crate) unsafe fn set_native_table_row_height(table: id, row_height: f64) {
    unsafe {
        let _: () = msg_send![table, setRowHeight: row_height];
    }
}

/// No-op on iOS (no row size style concept).
pub(crate) unsafe fn set_native_table_row_size_style(_table: id, _row_size_style: i64) {}

/// Sets table style (plain/grouped/inset).
pub(crate) unsafe fn set_native_table_style(_table: id, _style: i64) {
    // Style must be set at init time on iOS. No-op for now.
}

/// Sets selection highlight style.
pub(crate) unsafe fn set_native_table_selection_highlight_style(_table: id, _style: i64) {}

/// No-op on iOS.
pub(crate) unsafe fn set_native_table_grid_style(_table: id, _grid_style_mask: u64) {}

/// No-op on iOS.
pub(crate) unsafe fn set_native_table_uses_alternating_rows(_table: id, _uses: bool) {}

/// Sets whether multiple selection is allowed.
pub(crate) unsafe fn set_native_table_allows_multiple_selection(table: id, allows: bool) {
    unsafe {
        let _: () = msg_send![table, setAllowsMultipleSelection: allows as i8];
    }
}

/// No-op on iOS (no native table header in UITableView without sections).
pub(crate) unsafe fn set_native_table_show_header(_table: id, _show_header: bool) {}

/// Releases the table data source.
pub(crate) unsafe fn release_native_table_target(target: *mut c_void) {
    unsafe {
        if !target.is_null() {
            let source = target as id;
            let data_ptr: *mut c_void = *(*source).get_ivar(DATA_IVAR);
            if !data_ptr.is_null() {
                let _ = Box::from_raw(data_ptr as *mut Vec<IosTableRow>);
            }
            let _: () = msg_send![source, release];
        }
    }
}

/// Releases a UITableView.
pub(crate) unsafe fn release_native_table_view(table: id) {
    unsafe {
        if !table.is_null() {
            let _: () = msg_send![table, release];
        }
    }
}
