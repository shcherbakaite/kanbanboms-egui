#![doc = include_str!("../README.md")]

pub mod draw;
pub mod viewer;

pub use draw::Renderer;
pub use viewer::{ColumnHeaderMenuAction, RowViewer, UiAction};

/// You may want to sync egui version with this crate.
pub extern crate egui;

/* ---------------------------------------------------------------------------------------------- */
/*                                           CORE CLASS                                           */
/* ---------------------------------------------------------------------------------------------- */

/// Prevents direct modification of `Vec`
pub struct DataTable<R> {
    /// Efficient row data storage
    ///
    /// XXX: If we use `VecDeque` here, it'd be more efficient when inserting new element
    /// at the beginning of the list. However, it does not support `splice` method like
    /// `Vec`, which results in extremely inefficient when there's multiple insertions.
    ///
    /// The efficiency order of general operations are only twice as slow when using
    /// `Vec`, we're just ignoring it for now. Maybe we can utilize `IndexMap` for this
    /// purpose, however, there are many trade-offs to consider, for now, we're just
    /// using `Vec` for simplicity.
    rows: Vec<R>,

    /// Is Dirty?
    dirty_flag: bool,

    /// Ui
    ui: Option<Box<draw::state::UiState<R>>>,

    /// Clipboard preserved when ui is cleared (e.g. on replace/retain) so it can be
    /// restored when switching between tables (e.g. BOM edit windows).
    preserved_clipboard: Option<draw::state::Clipboard<R>>,
}

impl<R: std::fmt::Debug> std::fmt::Debug for DataTable<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spreadsheet")
            .field("rows", &self.rows)
            .finish()
    }
}

impl<R> Default for DataTable<R> {
    fn default() -> Self {
        Self {
            rows: Default::default(),
            ui: Default::default(),
            dirty_flag: false,
            preserved_clipboard: None,
        }
    }
}

impl<R> FromIterator<R> for DataTable<R> {
    fn from_iter<T: IntoIterator<Item = R>>(iter: T) -> Self {
        Self {
            rows: iter.into_iter().collect(),
            ..Default::default()
        }
    }
}

impl<R> DataTable<R> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &R> {
        self.rows.iter()
    }

    /// Returns rows in display order (respecting current sort from the data grid).
    /// If no sort is applied or UI state is unavailable, returns rows in storage order.
    pub fn iter_in_display_order(&self) -> Vec<&R> {
        if let Some(s) = &self.ui {
            let indices = s.sorted_row_indices();
            if !indices.is_empty() && indices.len() == self.rows.len() {
                return indices.iter().map(|&i| &self.rows[i]).collect();
            }
        }
        self.rows.iter().collect()
    }

    /// Returns sort state as (column_index, ascending) pairs. Primary sort is last.
    /// Empty if no sort is applied.
    pub fn sort_state(&self) -> Vec<(usize, bool)> {
        self.ui
            .as_ref()
            .map(|s| s.sort_state())
            .unwrap_or_default()
    }

    /// Returns column indices in display order. None if UI state is unavailable
    /// (e.g. table has not been rendered yet).
    pub fn column_order(&self) -> Option<Vec<usize>> {
        self.ui.as_ref().map(|s| s.column_order())
    }

    pub fn take(&mut self) -> Vec<R> {
        self.preserve_clipboard();
        self.ui = None;
        std::mem::take(&mut self.rows)
    }

    pub fn replace(&mut self, new: Vec<R>) -> Vec<R> {
        self.preserve_clipboard();
        self.ui = None;
        std::mem::replace(&mut self.rows, new)
    }

    /// Replaces rows while preserving UI state (column order, visibility, sort).
    /// Use when refreshing data without changing the table structure (column count).
    pub fn replace_keeping_ui(&mut self, new: Vec<R>) -> Vec<R> {
        let old = std::mem::replace(&mut self.rows, new);
        if let Some(ref mut ui) = self.ui {
            ui.force_mark_dirty();
        }
        old
    }

    pub fn retain(&mut self, mut f: impl FnMut(&R) -> bool) {
        let mut removed_any = false;
        self.rows.retain(|row| {
            let retain = f(row);
            removed_any |= !retain;
            retain
        });

        if removed_any {
            self.preserve_clipboard();
            self.ui = None;
        }
    }

    /// Save clipboard from ui into preserved_clipboard before clearing ui.
    fn preserve_clipboard(&mut self) {
        if let Some(ref mut ui) = self.ui {
            self.preserved_clipboard = ui.take_clipboard();
        }
    }

    pub fn clear_dirty_flag(&mut self) {
        self.dirty_flag = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_flag
    }

    pub fn has_user_modification(&self) -> bool {
        self.dirty_flag
    }

    pub fn clear_user_modification_flag(&mut self) {
        self.dirty_flag = false;
    }
}

impl<R> Extend<R> for DataTable<R> {
    /// Programmatic extend operation will invalidate the index table cache.
    fn extend<T: IntoIterator<Item = R>>(&mut self, iter: T) {
        // Invalidate the cache
        self.preserve_clipboard();
        self.ui = None;
        self.rows.extend(iter);
    }
}

fn default<T: Default>() -> T {
    T::default()
}
