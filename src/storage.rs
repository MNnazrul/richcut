use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    io,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::clipboard::NativeClipboardContent;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredEntry {
    Text { text: String, html: Option<String> },
    Html { html: String },
    Files { paths: Vec<String> },
    Image { file: String },
}

#[derive(Serialize, Deserialize)]
struct WindowPosition {
    x: f32,
    y: f32,
}

#[derive(Clone)]
pub struct Storage {
    dir: PathBuf,
}

impl Storage {
    pub fn new() -> Option<Self> {
        let home = std::env::var_os("HOME")?;

        Some(Self {
            dir: PathBuf::from(home).join("Library/Application Support/richcut"),
        })
    }

    fn history_path(&self) -> PathBuf {
        self.dir.join("history.json")
    }

    fn images_dir(&self) -> PathBuf {
        self.dir.join("images")
    }

    fn window_path(&self) -> PathBuf {
        self.dir.join("window.json")
    }

    pub fn load_window_position(&self) -> Option<(f32, f32)> {
        let data = fs::read(self.window_path()).ok()?;
        let position = serde_json::from_slice::<WindowPosition>(&data).ok()?;

        Some((position.x, position.y))
    }

    pub fn save_window_position(&self, x: f32, y: f32) -> io::Result<()> {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)?;

        fs::write(
            self.window_path(),
            serde_json::to_vec(&WindowPosition { x, y })?,
        )
    }

    pub fn load(&self) -> Vec<NativeClipboardContent> {
        let Ok(data) = fs::read(self.history_path()) else {
            return Vec::new();
        };

        let Ok(stored) = serde_json::from_slice::<Vec<StoredEntry>>(&data) else {
            return Vec::new();
        };

        stored
            .into_iter()
            .filter_map(|entry| match entry {
                StoredEntry::Text { text, html } => {
                    Some(NativeClipboardContent::Text { text, html })
                }
                StoredEntry::Html { html } => Some(NativeClipboardContent::Html(html)),
                StoredEntry::Files { paths } => Some(NativeClipboardContent::Files(paths)),
                StoredEntry::Image { file } => fs::read(self.images_dir().join(file))
                    .ok()
                    .map(NativeClipboardContent::ImageByte),
            })
            .collect()
    }

    pub fn save<'a>(
        &self,
        contents: impl Iterator<Item = &'a NativeClipboardContent>,
    ) -> io::Result<()> {
        let images_dir = self.images_dir();

        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&images_dir)?;

        let mut stored = Vec::new();
        let mut image_files = HashSet::new();

        for content in contents {
            let entry = match content {
                NativeClipboardContent::Text { text, html } => StoredEntry::Text {
                    text: text.clone(),
                    html: html.clone(),
                },
                NativeClipboardContent::Html(html) => StoredEntry::Html { html: html.clone() },
                NativeClipboardContent::Files(paths) => StoredEntry::Files {
                    paths: paths.clone(),
                },
                NativeClipboardContent::ImageByte(bytes) => {
                    let file = image_file_name(bytes);
                    let path = images_dir.join(&file);

                    if !path.exists() {
                        fs::write(&path, bytes)?;
                    }

                    image_files.insert(file.clone());
                    StoredEntry::Image { file }
                }
                NativeClipboardContent::Unknown => continue,
            };

            stored.push(entry);
        }

        let temp_path = self.dir.join("history.json.tmp");
        fs::write(&temp_path, serde_json::to_vec(&stored)?)?;
        fs::rename(&temp_path, self.history_path())?;

        for entry in fs::read_dir(&images_dir)?.flatten() {
            if !image_files.contains(entry.file_name().to_string_lossy().as_ref()) {
                let _ = fs::remove_file(entry.path());
            }
        }

        Ok(())
    }
}

fn image_file_name(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);

    format!("{:016x}-{}.tiff", hasher.finish(), bytes.len())
}
