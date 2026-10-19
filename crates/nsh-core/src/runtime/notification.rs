//! `nsh/notification`: system notifications (desktop toast notifications,
//! iOS / Android local notifications where supported).

use crate::ui::{ModuleCx, module};

/// System notifications.
pub(crate) struct Notification;

/// Shows a system notification.
#[module]
impl Notification {
    /// Shows a system notification with `title` and `body`.
    ///
    /// On desktops this is a toast notification; on mobile it posts to the
    /// notification center. Does nothing where notifications are unavailable.
    pub(crate) fn notify(title: String, body: String, cx: &mut ModuleCx) {
        cx.notify(title, body);
    }
}
