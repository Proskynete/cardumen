//! Dock icon badge: Claude sessions waiting for the user that they have not looked at.
use objc2_app_kit::NSApplication;
use objc2_foundation::{MainThreadMarker, NSString};

/// Show `count` on the Dock icon; 0 clears it.
pub fn set_badge(count: usize) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
    // SAFETY: main thread (checked above); setBadgeLabel copies the string.
    unsafe { NSApplication::sharedApplication(mtm).dockTile().setBadgeLabel(label.as_deref()) };
}
