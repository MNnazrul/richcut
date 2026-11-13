use std::{
    cell::OnceCell,
    fs,
    path::Path,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use slint::{Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};

use crate::{
    ClipRow, MainWindow, Preview, PreviewFile, clipboard::NativeClipboardContent, storage::Storage,
};

const MAX_HISTORY: usize = 100;
const MAX_TITLE_CHARS: usize = 200;
const MAX_PREVIEW_CHARS: usize = 10_000;
const MAX_PREVIEW_IMAGE: u32 = 800;
const MAX_THUMBNAIL: u32 = 96;

struct DecodedImage {
    image: slint::Image,
    width: u32,
    height: u32,
}

struct Entry {
    content: NativeClipboardContent,
    title: SharedString,
    kind: SharedString,
    subtitle: SharedString,
    icon: SharedString,
    search_key: String,
    copied_at: Option<u64>,
    image: OnceCell<Option<DecodedImage>>,
    thumbnail: OnceCell<Option<DecodedImage>>,
}

impl Entry {
    fn new(content: NativeClipboardContent, copied_at: Option<u64>) -> Option<Self> {
        let (kind, icon, title, subtitle, full) = match &content {
            NativeClipboardContent::Text { text, html } => {
                let (kind, icon) = if html.is_some() {
                    ("Rich text", "rich")
                } else {
                    ("Text", "text")
                };
                let subtitle = format!("{kind} · {}", text_summary(text));

                (
                    kind.to_string(),
                    icon,
                    first_line(text),
                    subtitle,
                    text.clone(),
                )
            }
            NativeClipboardContent::Html(html) => {
                let subtitle = format!("HTML · {}", text_summary(html));

                (
                    "HTML".to_string(),
                    "html",
                    first_line(html),
                    subtitle,
                    html.clone(),
                )
            }
            NativeClipboardContent::Files(paths) => {
                let is_folder = matches!(paths.as_slice(), [path] if Path::new(path).is_dir());
                let kind = match paths.len() {
                    1 if is_folder => "Folder".to_string(),
                    1 => "File".to_string(),
                    count => format!("{count} files"),
                };
                let names = paths
                    .iter()
                    .map(|path| file_name(path))
                    .collect::<Vec<_>>()
                    .join(", ");
                let location = paths
                    .first()
                    .and_then(|path| Path::new(path).parent())
                    .map(|parent| shorten_home(&parent.to_string_lossy()))
                    .unwrap_or_default();
                let subtitle = if paths.len() == 1 {
                    location
                } else {
                    format!("{kind} · {location}")
                };
                let icon = if is_folder { "folder" } else { "file" };

                (kind, icon, names, subtitle, paths.join("\n"))
            }
            NativeClipboardContent::ImageByte(bytes) => {
                let size = format_bytes(bytes.len() as u64);

                (
                    "Image".to_string(),
                    "image",
                    "Image".to_string(),
                    size.clone(),
                    format!("image {size}"),
                )
            }
            NativeClipboardContent::Unknown => return None,
        };

        if full.trim().is_empty() {
            return None;
        }

        Some(Self {
            title: truncate_chars(&title, MAX_TITLE_CHARS).into(),
            kind: kind.into(),
            subtitle: subtitle.into(),
            icon: icon.into(),
            search_key: full.to_lowercase(),
            copied_at,
            image: OnceCell::new(),
            thumbnail: OnceCell::new(),
            content,
        })
    }

    fn thumbnail(&self) -> Option<&DecodedImage> {
        let NativeClipboardContent::ImageByte(bytes) = &self.content else {
            return None;
        };

        self.thumbnail
            .get_or_init(|| decode_image(bytes, MAX_THUMBNAIL))
            .as_ref()
    }

    fn preview(&self) -> Preview {
        let mut preview = Preview {
            kind: self.kind.clone(),
            ..Default::default()
        };
        let mut meta = Vec::new();

        match &self.content {
            NativeClipboardContent::Text { text, html } => {
                meta.extend(text_stats(text));

                if html.is_some() {
                    meta.push("with formatting".to_string());
                }

                preview.text = truncate_chars(text, MAX_PREVIEW_CHARS).into();
            }
            NativeClipboardContent::Html(html) => {
                meta.extend(text_stats(html));
                preview.text = truncate_chars(html, MAX_PREVIEW_CHARS).into();
            }
            NativeClipboardContent::Files(paths) => {
                if let [path] = paths.as_slice() {
                    meta.push(match fs::metadata(path) {
                        Ok(metadata) if metadata.is_dir() => "Folder".to_string(),
                        Ok(metadata) => format_bytes(metadata.len()),
                        Err(_) => "Missing".to_string(),
                    });
                }

                let files: Vec<PreviewFile> = paths.iter().map(|path| preview_file(path)).collect();
                preview.files = ModelRc::new(VecModel::from(files));
            }
            NativeClipboardContent::ImageByte(bytes) => {
                match self
                    .image
                    .get_or_init(|| decode_image(bytes, MAX_PREVIEW_IMAGE))
                {
                    Some(decoded) => {
                        meta.push(format!("{} × {} px", decoded.width, decoded.height));
                        preview.image = decoded.image.clone();
                        preview.is_image = true;
                    }
                    None => preview.text = "Image preview unavailable".into(),
                }

                meta.push(format_bytes(bytes.len() as u64));
            }
            NativeClipboardContent::Unknown => {}
        }

        if let Some(copied_at) = self.copied_at {
            meta.push(relative_time(copied_at));
        }

        preview.meta = meta.join(" · ").into();
        preview
    }
}

pub struct History {
    entries: Vec<Entry>,
    rows: Rc<VecModel<ClipRow>>,
    query: String,
    storage: Option<Storage>,
}

impl History {
    pub fn new(rows: Rc<VecModel<ClipRow>>, storage: Option<Storage>) -> Self {
        let mut entries: Vec<Entry> = Vec::new();

        for (content, copied_at) in storage.as_ref().map(Storage::load).unwrap_or_default() {
            let Some(entry) = Entry::new(content, copied_at) else {
                continue;
            };

            if entries
                .iter()
                .any(|existing| existing.content == entry.content)
            {
                continue;
            }

            entries.push(entry);

            if entries.len() == MAX_HISTORY {
                break;
            }
        }

        Self {
            entries,
            rows,
            query: String::new(),
            storage,
        }
    }

    pub fn content(&self, source: usize) -> Option<&NativeClipboardContent> {
        self.entries.get(source).map(|entry| &entry.content)
    }

    pub fn source_at_row(&self, row: i32) -> Option<usize> {
        let row = usize::try_from(row).ok()?;
        let source = self.rows.row_data(row)?.source;

        usize::try_from(source).ok()
    }

    pub fn push_front(&mut self, content: NativeClipboardContent) -> bool {
        if self
            .entries
            .first()
            .map(|entry| entry.content == content)
            .unwrap_or(false)
        {
            return false;
        }

        let now = now_secs();

        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.content == content)
        {
            let mut entry = self.entries.remove(index);
            entry.copied_at = Some(now);
            self.entries.insert(0, entry);
        } else {
            let Some(entry) = Entry::new(content, Some(now)) else {
                return false;
            };

            self.entries.insert(0, entry);
            self.entries.truncate(MAX_HISTORY);
        }

        self.save();

        true
    }

    pub fn move_to_front(&mut self, source: usize) {
        if source >= self.entries.len() {
            return;
        }

        let mut entry = self.entries.remove(source);
        entry.copied_at = Some(now_secs());
        self.entries.insert(0, entry);
        self.save();
    }

    pub fn remove(&mut self, source: usize) -> bool {
        if source >= self.entries.len() {
            return false;
        }

        self.entries.remove(source);
        self.save();

        true
    }

    pub fn clear(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        self.entries.clear();
        self.save();
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
    }

    pub fn refresh(&self, app: &MainWindow, keep_source: Option<usize>) {
        let rows = self.visible_rows();

        let selected = keep_source
            .and_then(|source| rows.iter().position(|row| row.source == source as i32))
            .or(if rows.is_empty() { None } else { Some(0) });

        self.show_rows(app, rows, selected);
    }

    pub fn refresh_at_row(&self, app: &MainWindow, row: usize) {
        let rows = self.visible_rows();
        let selected = rows.len().checked_sub(1).map(|last| row.min(last));

        self.show_rows(app, rows, selected);
    }

    pub fn update_preview(&self, app: &MainWindow) {
        let preview = self
            .source_at_row(app.get_selected_index())
            .and_then(|source| self.entries.get(source))
            .map(Entry::preview)
            .unwrap_or_default();

        app.set_preview(preview);
    }

    fn visible_rows(&self) -> Vec<ClipRow> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.query.is_empty() || entry.search_key.contains(&self.query))
            .map(|(source, entry)| {
                let thumbnail = entry.thumbnail();
                let subtitle = match thumbnail {
                    Some(thumbnail) => format!(
                        "{} × {} px · {}",
                        thumbnail.width, thumbnail.height, entry.subtitle
                    )
                    .into(),
                    None => entry.subtitle.clone(),
                };

                ClipRow {
                    title: entry.title.clone(),
                    subtitle,
                    icon: entry.icon.clone(),
                    time: entry.copied_at.map(short_time).unwrap_or_default().into(),
                    thumbnail: thumbnail
                        .map(|thumbnail| thumbnail.image.clone())
                        .unwrap_or_default(),
                    has_thumbnail: thumbnail.is_some(),
                    source: source as i32,
                }
            })
            .collect()
    }

    fn show_rows(&self, app: &MainWindow, rows: Vec<ClipRow>, selected: Option<usize>) {
        app.set_history_count(self.entries.len() as i32);
        self.rows.set_vec(rows);
        app.set_selected_index(selected.map_or(-1, |row| row as i32));
        app.invoke_ensure_selected_visible();
        self.update_preview(app);
    }

    fn save(&self) {
        let Some(storage) = &self.storage else {
            return;
        };

        let records = self
            .entries
            .iter()
            .map(|entry| (&entry.content, entry.copied_at));

        if let Err(error) = storage.save(records) {
            eprintln!("richcut: failed to save history: {error}");
        }
    }
}

fn decode_image(bytes: &[u8], max_size: u32) -> Option<DecodedImage> {
    let decoded = image::load_from_memory(bytes).ok()?;
    let (width, height) = (decoded.width(), decoded.height());

    let scaled = if width > max_size || height > max_size {
        decoded.thumbnail(max_size, max_size)
    } else {
        decoded
    };
    let rgba = scaled.to_rgba8();
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
    );

    Some(DecodedImage {
        image: slint::Image::from_rgba8(buffer),
        width,
        height,
    })
}

fn preview_file(path: &str) -> PreviewFile {
    let metadata = fs::metadata(path).ok();
    let name = file_name(path);
    let name = if metadata.as_ref().is_some_and(fs::Metadata::is_dir) {
        format!("{name}/")
    } else {
        name
    };
    let location = Path::new(path)
        .parent()
        .map(|parent| shorten_home(&parent.to_string_lossy()))
        .unwrap_or_default();

    PreviewFile {
        name: name.into(),
        location: location.into(),
        missing: metadata.is_none(),
    }
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map_or_else(
        || path.to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn shorten_home(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}

fn text_summary(text: &str) -> String {
    let lines = text.lines().count();

    if lines > 1 {
        format!("{} lines", group_digits(lines))
    } else {
        let chars = text.chars().count();
        format!("{} {}", group_digits(chars), plural(chars, "char", "chars"))
    }
}

fn text_stats(text: &str) -> [String; 2] {
    let chars = text.chars().count();
    let lines = text.lines().count().max(1);

    [
        format!("{} {}", group_digits(chars), plural(chars, "char", "chars")),
        format!("{} {}", group_digits(lines), plural(lines, "line", "lines")),
    ]
}

fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

fn group_digits(value: usize) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }

    grouped
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut size = bytes as f64 / 1024.0;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    format!("{size:.1} {}", UNITS[unit])
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn relative_time(copied_at: u64) -> String {
    let elapsed = now_secs().saturating_sub(copied_at);

    match elapsed {
        0..60 => "just now".to_string(),
        60..3_600 => format!("{} min ago", elapsed / 60),
        3_600..86_400 => format!("{} h ago", elapsed / 3_600),
        86_400..2_592_000 => {
            let days = elapsed / 86_400;
            format!("{days} {} ago", plural(days as usize, "day", "days"))
        }
        _ => format!("{} mo ago", elapsed / 2_592_000),
    }
}

fn short_time(copied_at: u64) -> String {
    let elapsed = now_secs().saturating_sub(copied_at);

    match elapsed {
        0..60 => "now".to_string(),
        60..3_600 => format!("{}m", elapsed / 60),
        3_600..86_400 => format!("{}h", elapsed / 3_600),
        86_400..2_592_000 => format!("{}d", elapsed / 86_400),
        _ => format!("{}mo", elapsed / 2_592_000),
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn truncate_chars(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((index, _)) => format!("{}…", &text[..index]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> NativeClipboardContent {
        NativeClipboardContent::Text {
            text: value.to_string(),
            html: None,
        }
    }

    fn history() -> History {
        let rows = Rc::new(VecModel::<ClipRow>::default());

        History::new(rows, None)
    }

    fn contents(history: &History) -> Vec<String> {
        history
            .entries
            .iter()
            .map(|entry| match &entry.content {
                NativeClipboardContent::Text { text, .. } => text.clone(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn repeated_copy_moves_to_front_without_duplicating() {
        let mut history = history();

        assert!(history.push_front(text("a")));
        assert!(history.push_front(text("b")));
        assert!(history.push_front(text("a")));

        assert_eq!(contents(&history), ["a", "b"]);
    }

    #[test]
    fn copying_the_newest_item_again_is_a_no_op() {
        let mut history = history();

        assert!(history.push_front(text("a")));
        assert!(!history.push_front(text("a")));
        assert_eq!(contents(&history), ["a"]);
    }

    #[test]
    fn repeated_copy_keeps_the_entry_at_the_front() {
        let mut history = history();

        for value in ["a", "b", "c", "b", "a", "c"] {
            history.push_front(text(value));
        }

        assert_eq!(contents(&history), ["c", "a", "b"]);
    }

    #[test]
    fn dedupe_respects_the_history_cap() {
        let mut history = history();

        for index in 0..MAX_HISTORY {
            history.push_front(text(&format!("item-{index}")));
        }

        assert_eq!(history.entries.len(), MAX_HISTORY);

        history.push_front(text("item-0"));

        assert_eq!(history.entries.len(), MAX_HISTORY);
        assert_eq!(contents(&history)[0], "item-0");
    }
}
