use std::time::Duration;

use slint::{Model, ModelRc, VecModel};

use objc2::ClassType;
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeHTML, NSPasteboardTypeString, NSPasteboardTypeTIFF,
};

use objc2_foundation::{NSArray, NSURL};
slint::include_modules!();

#[derive(Debug)]
pub enum NativeClipboardContent {
    Text(String),
    Html(String),
    Files(Vec<String>),
    ImageByte(Vec<u8>),
    Unknown,
}

impl NativeClipboardContent {
    fn into_display_text(self) -> Option<String> {
        match self {
            Self::Text(text) if !text.trim().is_empty() => Some(text),
            Self::Html(html) if !html.trim().is_empty() => Some(html),
            Self::Files(files) if !files.is_empty() => Some(format!("{}", files.join(", "))),
            Self::ImageByte(bytes) if !bytes.is_empty() => {
                Some(format!("Image ({} bytes)", bytes.len()))
            }
            _ => None,
        }
    }
}

fn fetch_clipboard_data(pasteboard: &NSPasteboard) -> NativeClipboardContent {
    unsafe {
        if let Some(objects) =
            pasteboard.readObjectsForClasses_options(&NSArray::from_slice(&[NSURL::class()]), None)
        {
            let mut file_paths = Vec::new();

            for obj in objects.iter() {
                if let Ok(url) = obj.downcast::<NSURL>() {
                    if let Some(path) = url.path() {
                        file_paths.push(path.to_string());
                    }
                }
            }

            if !file_paths.is_empty() {
                return NativeClipboardContent::Files(file_paths);
            }
        }
        if let Some(html) = pasteboard.stringForType(NSPasteboardTypeHTML) {
            let html = html.to_string();

            if !html.trim().is_empty() {
                return NativeClipboardContent::Html(html);
            }
        }

        if let Some(text) = pasteboard.stringForType(NSPasteboardTypeString) {
            let text = text.to_string();

            if !text.trim().is_empty() {
                return NativeClipboardContent::Text(text);
            }
        }

        if let Some(data) = pasteboard.dataForType(NSPasteboardTypeTIFF) {
            let bytes = data.as_bytes_unchecked().to_vec();

            if !bytes.is_empty() {
                return NativeClipboardContent::ImageByte(bytes);
            }
        }
    }

    NativeClipboardContent::Unknown
}

fn main() -> Result<(), slint::PlatformError> {
    let app = MainWindow::new()?;

    let history = std::rc::Rc::new(VecModel::<slint::SharedString>::default());

    app.set_clipboard_items(ModelRc::from(history.clone()));

    let pasteboard = NSPasteboard::generalPasteboard();
    let mut last_change = pasteboard.changeCount();

    let timer = slint::Timer::default();

    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(150),
        move || {
            let current_count = pasteboard.changeCount();
            if current_count == last_change {
                return;
            }

            last_change = current_count;
            let content = fetch_clipboard_data(&pasteboard);

            let Some(text) = content.into_display_text() else {
                return;
            };

            if history.row_data(0).as_deref() == Some(text.as_str()) {
                return;
            }

            // Newest item first.
            history.insert(0, text.into());

            // Keep the history bounded.
            if history.row_count() > 100 {
                history.remove(history.row_count() - 1);
            }
        },
    );

    app.run()
}
