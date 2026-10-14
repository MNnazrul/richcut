use std::{thread::sleep, time::Duration};

use objc2::ClassType;

use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeHTML, NSPasteboardTypeString, NSPasteboardTypeTIFF,
};
use objc2_foundation::{NSArray, NSURL};

#[derive(Debug)]
pub enum NativeClipboardContent {
    Text(String),
    Html(String),
    Files(Vec<String>),
    ImageByte(Vec<u8>),
    Unknown,
}

fn fetch_and_save_clipboard_data(pasteboard: &NSPasteboard) -> NativeClipboardContent {
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
                } else {
                    continue;
                }
            }
            if !file_paths.is_empty() {
                for file in file_paths {
                    println!("file path {file}");
                }
            }
        }
        if let Some(html_ns) = pasteboard.stringForType(NSPasteboardTypeHTML) {
            let html = html_ns.to_string();
            if !html.trim().is_empty() {
                println!("{}", html);
            }
        }

        if let Some(text_ns) = pasteboard.stringForType(NSPasteboardTypeString) {
            let text = text_ns.to_string();
            if !text.trim().is_empty() {
                println!("{}", text);
            }
        }

        if let Some(data) = pasteboard.dataForType(NSPasteboardTypeTIFF) {
            let bytes = data.as_bytes_unchecked().to_vec();
            if !bytes.is_empty() {
                todo!()
            }
        }
    }

    NativeClipboardContent::Unknown
}

fn monitor_clipboard() {
    let pasteboard = NSPasteboard::generalPasteboard();
    let mut last_change_count = pasteboard.changeCount();

    loop {
        let current_change_count = pasteboard.changeCount();
        if current_change_count != last_change_count {
            last_change_count = current_change_count;
            fetch_and_save_clipboard_data(&pasteboard);
        }

        sleep(Duration::from_millis(100));
    }
}

fn main() {
    monitor_clipboard();
}
