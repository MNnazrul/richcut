use std::{path::Path, rc::Rc};

use slint::{Model, SharedString, VecModel};

use crate::{ClipRow, MainWindow, clipboard::NativeClipboardContent, storage::Storage};

const MAX_HISTORY: usize = 100;
const MAX_TITLE_CHARS: usize = 200;
const MAX_DETAIL_CHARS: usize = 2000;

struct Entry {
    content: NativeClipboardContent,
    title: SharedString,
    kind: SharedString,
    detail: SharedString,
    search_key: String,
}

impl Entry {
    fn new(content: NativeClipboardContent) -> Option<Self> {
        let (kind, title, full) = match &content {
            NativeClipboardContent::Text { text, html } => {
                let kind = if html.is_some() { "Rich text" } else { "Text" };
                (kind.to_string(), first_line(text), text.clone())
            }
            NativeClipboardContent::Html(html) => {
                ("HTML".to_string(), first_line(html), html.clone())
            }
            NativeClipboardContent::Files(paths) => {
                let kind = if paths.len() == 1 {
                    "File".to_string()
                } else {
                    format!("{} files", paths.len())
                };
                let names = paths
                    .iter()
                    .map(|path| {
                        Path::new(path).file_name().map_or_else(
                            || path.clone(),
                            |name| name.to_string_lossy().into_owned(),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                (kind, names, paths.join("\n"))
            }
            NativeClipboardContent::ImageByte(bytes) => {
                let label = format!("Image ({} bytes)", bytes.len());
                ("Image".to_string(), label.clone(), label)
            }
            NativeClipboardContent::Unknown => return None,
        };

        if full.trim().is_empty() {
            return None;
        }

        Some(Self {
            title: truncate_chars(&title, MAX_TITLE_CHARS).into(),
            kind: kind.into(),
            detail: truncate_chars(&full, MAX_DETAIL_CHARS).into(),
            search_key: full.to_lowercase(),
            content,
        })
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
        let entries = storage
            .as_ref()
            .map(Storage::load)
            .unwrap_or_default()
            .into_iter()
            .filter_map(Entry::new)
            .take(MAX_HISTORY)
            .collect();

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
        if self.entries.first().map(|entry| &entry.content) == Some(&content) {
            return false;
        }

        let Some(entry) = Entry::new(content) else {
            return false;
        };

        self.entries.insert(0, entry);
        self.entries.truncate(MAX_HISTORY);
        self.save();

        true
    }

    pub fn move_to_front(&mut self, source: usize) {
        if source == 0 || source >= self.entries.len() {
            return;
        }

        let entry = self.entries.remove(source);
        self.entries.insert(0, entry);
        self.save();
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
    }

    pub fn refresh(&self, app: &MainWindow, keep_source: Option<usize>) {
        let rows: Vec<ClipRow> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| self.query.is_empty() || entry.search_key.contains(&self.query))
            .map(|(source, entry)| ClipRow {
                title: entry.title.clone(),
                kind: entry.kind.clone(),
                detail: entry.detail.clone(),
                source: source as i32,
            })
            .collect();

        let selected = keep_source
            .and_then(|source| rows.iter().position(|row| row.source == source as i32))
            .or(if rows.is_empty() { None } else { Some(0) })
            .map_or(-1, |row| row as i32);

        self.rows.set_vec(rows);
        app.set_selected_index(selected);
        app.invoke_ensure_selected_visible();
    }

    fn save(&self) {
        let Some(storage) = &self.storage else {
            return;
        };

        if let Err(error) = storage.save(self.entries.iter().map(|entry| &entry.content)) {
            eprintln!("richcut: failed to save history: {error}");
        }
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
