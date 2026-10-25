use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSApplicationActivationPolicy, NSEvent,
    NSRunningApplication, NSScreen, NSView, NSWindowCollectionBehavior, NSWorkspace,
};
use objc2_application_services::{AXIsProcessTrusted, AXIsProcessTrustedWithOptions};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};
use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventTapLocation};
use objc2_foundation::NSRect;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const KEY_CODE_V: u16 = 9;

pub fn set_accessory_policy(mtm: MainThreadMarker) {
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

pub fn activate_self(mtm: MainThreadMarker) {
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
}

pub fn is_self_active(mtm: MainThreadMarker) -> bool {
    NSApplication::sharedApplication(mtm).isActive()
}

pub fn frontmost_other_app() -> Option<Retained<NSRunningApplication>> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;

    if app.processIdentifier() == std::process::id() as i32 {
        return None;
    }

    Some(app)
}

pub fn activate_app(app: &NSRunningApplication) {
    app.activateWithOptions(NSApplicationActivationOptions::empty());
}

pub fn request_accessibility() -> bool {
    let key: &CFString = unsafe { objc2_application_services::kAXTrustedCheckOptionPrompt };
    let options = CFDictionary::<CFString, CFBoolean>::from_slices(&[key], &[CFBoolean::new(true)]);

    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
}

pub fn is_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn send_paste_keystroke() {
    for key_down in [true, false] {
        let Some(event) = CGEvent::new_keyboard_event(None, KEY_CODE_V, key_down) else {
            return;
        };

        CGEvent::set_flags(Some(&event), CGEventFlags::MaskCommand);
        CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&event));
    }
}

pub fn picker_position(
    mtm: MainThreadMarker,
    width: f64,
    height: f64,
) -> Option<slint::LogicalPosition> {
    let mouse = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    let primary_height = screens.firstObject()?.frame().size.height;

    let visible = screens
        .iter()
        .map(|screen| (screen.frame(), screen.visibleFrame()))
        .find(|(frame, _)| contains(frame, mouse.x, mouse.y))
        .map(|(_, visible)| visible)
        .or_else(|| screens.firstObject().map(|screen| screen.visibleFrame()))?;

    let min_x = visible.origin.x;
    let max_x = (visible.origin.x + visible.size.width - width).max(min_x);
    let min_top = visible.origin.y + height;
    let max_top = (visible.origin.y + visible.size.height).max(min_top);

    let x = mouse.x.clamp(min_x, max_x);
    let top = mouse.y.clamp(min_top, max_top);

    Some(slint::LogicalPosition::new(
        x as f32,
        (primary_height - top) as f32,
    ))
}

pub fn is_on_screen(mtm: MainThreadMarker, position: slint::LogicalPosition) -> bool {
    let screens = NSScreen::screens(mtm);
    let Some(primary) = screens.firstObject() else {
        return false;
    };

    let x = f64::from(position.x) + 40.0;
    let y = primary.frame().size.height - f64::from(position.y) - 17.0;

    screens
        .iter()
        .any(|screen| contains(&screen.visibleFrame(), x, y))
}

pub fn configure_window(window: &slint::Window) {
    let handle = window.window_handle();
    let Ok(raw) = handle.window_handle() else {
        return;
    };

    let RawWindowHandle::AppKit(appkit) = raw.as_raw() else {
        return;
    };

    let view: &NSView = unsafe { appkit.ns_view.cast().as_ref() };

    if let Some(ns_window) = view.window() {
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::MoveToActiveSpace
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
    }
}

fn contains(rect: &NSRect, x: f64, y: f64) -> bool {
    x >= rect.origin.x
        && x < rect.origin.x + rect.size.width
        && y >= rect.origin.y
        && y < rect.origin.y + rect.size.height
}
