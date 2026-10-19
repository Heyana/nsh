//! `nsh/shell`: opening URLs in the system's default browser.

use crate::{
    Command, ROOT_ID,
    desktop::DesktopCommand,
    ui::{ModuleCx, module, task::Action},
};

/// Shell integration: opening URLs outside the app.
pub(crate) struct Shell;

/// Opens URLs in the system's default browser (desktops; nothing happens
/// elsewhere).
#[module]
impl Shell {
    /// Opens `url` in the system's default browser (desktops; its
    /// functions do nothing elsewhere).
    pub(crate) fn open_external(url: String, cx: &mut ModuleCx) {
        cx.act(Action::Command(
            ROOT_ID,
            Command::Desktop(Box::new(DesktopCommand::OpenExternal(url))),
        ));
    }
}
