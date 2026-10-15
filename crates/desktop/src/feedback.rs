//! Shared in-app operation feedback. Keep placement consistent across views.
use gpui_kit::component::{
    WindowExt,
    notification::{Notification, NotificationDelivery},
};
use gpui_kit::{Anchor, App, Window};

pub fn show(notification: Notification, window: &mut Window, cx: &mut App) {
    // Views may report errors while being created; wait until the root is available.
    window.defer(cx, move |window, cx| {
        window.push_notification(
            notification
                .placement(Anchor::BottomRight)
                .delivery(NotificationDelivery::InApp),
            cx,
        );
    });
}
