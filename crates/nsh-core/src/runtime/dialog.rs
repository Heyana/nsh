//! `nsh/dialog`: the system file picker (open / save dialogs).
//! Desktop-only; on other platforms the promises resolve to `null`.

use crate::{desktop::FileDialogOptions, ui::module};

/// The system file picker.
pub(crate) struct Dialog;

/// The system file picker.
#[module]
impl Dialog {
    /// Shows the system's open-file dialog. Resolves to the chosen paths,
    /// or `null` when the user cancels. `options.multiple` allows several.
    pub(crate) async fn open_file(options: Option<FileDialogOptions>) -> Option<Vec<String>> {
        crate::ui::prelude::dialog_open_file(options.unwrap_or_default()).await
    }

    /// Shows the system's save-file dialog. Resolves to the chosen path,
    /// or `null` when the user cancels.
    pub(crate) async fn save_file(options: Option<FileDialogOptions>) -> Option<String> {
        let paths = crate::ui::prelude::dialog_save_file(options.unwrap_or_default()).await;
        // Save dialogs choose one file; take it.
        paths.and_then(|mut p| p.pop())
    }
}
