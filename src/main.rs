use eframe::egui;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::SystemTime;

#[derive(Clone, Serialize, Deserialize)]
struct SearchCriteria {
    image_extensions: Vec<String>,
    media_extensions: Vec<String>,
    text_extensions: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AppConfig {
    language_extensions: HashMap<String, String>,
    search_criteria: SearchCriteria,
    included_files: Vec<String>,
    excluded_files: Vec<String>,
    ignored_folders: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut language_extensions = HashMap::new();
        let extensions = [
            ("rs", "Rust"),
            ("py", "Python"),
            ("js", "JavaScript"),
            ("ts", "TypeScript"),
            ("md", "Markdown"),
            ("toml", "TOML Config"),
            ("json", "JSON Data"),
            ("yaml", "YAML Config"),
            ("yml", "YAML Config"),
            ("xml", "XML Document"),
            ("html", "HTML Markup"),
            ("css", "Cascading Style Sheets"),
            ("c", "C Source"),
            ("cpp", "C++ Source"),
            ("h", "C/C++ Header"),
        ];

        for (ext, name) in extensions {
            language_extensions.insert(ext.to_string(), name.to_string());
        }

        Self {
            language_extensions,
            search_criteria: SearchCriteria {
                image_extensions: vec![
                    "png".into(),
                    "jpg".into(),
                    "jpeg".into(),
                    "webp".into(),
                    "bmp".into(),
                    "gif".into(),
                    "tiff".into(),
                ],
                media_extensions: vec![
                    "mp3".into(),
                    "mp4".into(),
                    "mkv".into(),
                    "wav".into(),
                    "flac".into(),
                    "ogg".into(),
                    "m4a".into(),
                    "aac".into(),
                    "avi".into(),
                    "mov".into(),
                    "webm".into(),
                    "opus".into(),
                ],
                text_extensions: vec![
                    "rs".into(),
                    "txt".into(),
                    "md".into(),
                    "toml".into(),
                    "json".into(),
                    "js".into(),
                    "ts".into(),
                    "py".into(),
                    "html".into(),
                    "css".into(),
                    "c".into(),
                    "cpp".into(),
                    "h".into(),
                    "xml".into(),
                    "yaml".into(),
                    "yml".into(),
                ],
            },
            included_files: vec!["important.txt".into(), "config.toml".into()],
            excluded_files: vec![
                ".DS_Store".into(),
                "Thumbs.db".into(),
                "desktop.ini".into(),
                "*.meta".into(),
                "*.log".into(),
            ],
            ignored_folders: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                ".vs".into(),
                ".idea".into(),
                "dist".into(),
                "build".into(),
                "Library".into(),
                "Logs".into(),
                "Obj".into(),
                "UserSettings".into(),
            ],
        }
    }
}

const WIDTH: f32 = 1100.0;
const HEIGHT: f32 = 780.0;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WIDTH, HEIGHT])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Advanced File Inspector",
        options,
        Box::new(|_cc| Ok(Box::new(FileInspectorApp::default()))),
    )
}

#[derive(Clone, Serialize, Deserialize)]
struct FileMetadataInfo {
    path: PathBuf,
    name: String,
    extension: String,
    size_bytes: u64,
    file_type: String,
    created_at: String,
    modified_at: String,
    extra_details: Vec<(String, String)>,
    file_hash: String,
    cached_at: String,
    is_directory: bool,
    tags: Vec<String>,
}

enum IndexProgress {
    Progress(String),
    Finished {
        total_files: usize,
        indexed_files: usize,
        unindexed_types: HashMap<String, usize>,
        unknown_types: HashMap<String, usize>,
    },
}

struct FileInspectorApp {
    current_file: Option<FileMetadataInfo>,
    status_message: String,
    db_path: PathBuf,
    config_path: PathBuf,
    config: AppConfig,
    tag_input_buffer: String,
    search_query: String,
    is_indexing: bool,
    indexing_rx: Option<Receiver<IndexProgress>>,
    current_indexing_file: String,
}

impl Default for FileInspectorApp {
    fn default() -> Self {
        let exe_path = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
        let db_path = exe_dir.join("file_inspector.db");
        let config_path = exe_dir.join("config.json");

        let config_content = fs::read_to_string(&config_path).unwrap_or_else(|_| {
            let default_cfg = AppConfig::default();
            let serialized = serde_json::to_string_pretty(&default_cfg).unwrap();
            let _ = fs::write(&config_path, &serialized);
            serialized
        });
        let config: AppConfig = serde_json::from_str(&config_content).unwrap_or_default();

        let app = Self {
            current_file: None,
            status_message: format!(
                "Loaded config & initialized SQLite DB: {}",
                db_path.display()
            ),
            db_path,
            config_path,
            config,
            tag_input_buffer: String::new(),
            search_query: String::new(),
            is_indexing: false,
            indexing_rx: None,
            current_indexing_file: String::new(),
        };

        app.init_db();
        app
    }
}

impl FileInspectorApp {
    fn init_db(&self) {
        if let Ok(conn) = Connection::open(&self.db_path) {
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS files (
                    file_hash TEXT PRIMARY KEY,
                    path TEXT NOT NULL,
                    name TEXT NOT NULL,
                    extension TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    file_type TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    modified_at TEXT NOT NULL,
                    extra_details TEXT NOT NULL,
                    cached_at TEXT NOT NULL,
                    is_directory BOOLEAN NOT NULL,
                    tags TEXT NOT NULL
                )",
                [],
            );
        }
    }

    fn truncate_name(name: &str, max_len: usize) -> String {
        if name.chars().count() > max_len {
            let truncated: String = name.chars().take(max_len).collect();
            format!("{}...", truncated)
        } else {
            name.to_string()
        }
    }

    fn reload_config(&mut self) {
        let content = fs::read_to_string(&self.config_path).unwrap_or_default();
        if let Ok(parsed) = serde_json::from_str::<AppConfig>(&content) {
            self.config = parsed;
            self.status_message =
                "Configuration reloaded successfully from config.json.".to_string();
        } else {
            self.status_message = "Failed to reload configuration (syntax error).".to_string();
        }
    }

    fn save_to_db(&self, info: &FileMetadataInfo) {
        if let Ok(conn) = Connection::open(&self.db_path) {
            let extra_json = serde_json::to_string(&info.extra_details).unwrap_or_default();
            let tags_json = serde_json::to_string(&info.tags).unwrap_or_default();
            let _ = conn.execute(
                "INSERT OR REPLACE INTO files (file_hash, path, name, extension, size_bytes, file_type, created_at, modified_at, extra_details, cached_at, is_directory, tags)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    info.file_hash,
                    info.path.to_string_lossy().to_string(),
                    info.name,
                    info.extension,
                    info.size_bytes as i64,
                    info.file_type,
                    info.created_at,
                    info.modified_at,
                    extra_json,
                    info.cached_at,
                    info.is_directory,
                    tags_json
                ],
            );
        }
    }

    fn get_from_db(&self, hash: &str) -> Option<FileMetadataInfo> {
        let conn = Connection::open(&self.db_path).ok()?;
        let mut stmt = conn
            .prepare(
                "SELECT path, name, extension, size_bytes, file_type, created_at, modified_at, extra_details, cached_at, is_directory, tags FROM files WHERE file_hash = ?",
            )
            .ok()?;

        let mut rows = stmt
            .query_map(params![hash], |row| {
                let path_str: String = row.get(0)?;
                let name: String = row.get(1)?;
                let extension: String = row.get(2)?;
                let size_bytes: i64 = row.get(3)?;
                let file_type: String = row.get(4)?;
                let created_at: String = row.get(5)?;
                let modified_at: String = row.get(6)?;
                let extra_json: String = row.get(7)?;
                let cached_at: String = row.get(8)?;
                let is_directory: bool = row.get(9)?;
                let tags_json: String = row.get(10)?;

                Ok(FileMetadataInfo {
                    path: PathBuf::from(path_str),
                    name,
                    extension,
                    size_bytes: size_bytes as u64,
                    file_type,
                    created_at,
                    modified_at,
                    extra_details: serde_json::from_str(&extra_json).unwrap_or_default(),
                    file_hash: hash.to_string(),
                    cached_at,
                    is_directory,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                })
            })
            .ok()?;

        rows.next().and_then(|r| r.ok())
    }

    fn fetch_all_from_db(&self) -> Vec<FileMetadataInfo> {
        let mut list = Vec::new();
        if let Ok(conn) = Connection::open(&self.db_path) {
            if let Ok(mut stmt) = conn.prepare(
                "SELECT file_hash, path, name, extension, size_bytes, file_type, created_at, modified_at, extra_details, cached_at, is_directory, tags FROM files"
            ) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    let file_hash: String = row.get(0)?;
                    let path_str: String = row.get(1)?;
                    let name: String = row.get(2)?;
                    let extension: String = row.get(3)?;
                    let size_bytes: i64 = row.get(4)?;
                    let file_type: String = row.get(5)?;
                    let created_at: String = row.get(6)?;
                    let modified_at: String = row.get(7)?;
                    let extra_json: String = row.get(8)?;
                    let cached_at: String = row.get(9)?;
                    let is_directory: bool = row.get(10)?;
                    let tags_json: String = row.get(11)?;

                    Ok(FileMetadataInfo {
                        path: PathBuf::from(path_str),
                        name,
                        extension,
                        size_bytes: size_bytes as u64,
                        file_type,
                        created_at,
                        modified_at,
                        extra_details: serde_json::from_str(&extra_json).unwrap_or_default(),
                        file_hash,
                        cached_at,
                        is_directory,
                        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                    })
                }) {
                    for r in rows.flatten() {
                        list.push(r);
                    }
                }
            }
        }
        list
    }

    fn export_to_json(&mut self, items: &[FileMetadataInfo], label: &str) {
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&format!("exported_{}.json", label))
            .save_file()
        {
            if let Ok(json_data) = serde_json::to_string_pretty(items) {
                if fs::write(&path, json_data).is_ok() {
                    self.status_message = format!(
                        "Successfully exported {} items to {}",
                        items.len(),
                        path.display()
                    );
                } else {
                    self.status_message = "Failed to write export file.".to_string();
                }
            } else {
                self.status_message = "Failed to serialize items to JSON.".to_string();
            }
        }
    }

    fn inspect_path(&mut self, path: PathBuf) {
        self.reload_config();

        if !path.exists() {
            self.status_message = "Error: Path does not exist.".to_string();
            return;
        }

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                self.status_message = format!("Error reading metadata: {}", e);
                return;
            }
        };

        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        if metadata.is_dir() {
            let name = file_name;
            let created_at = metadata
                .created()
                .map(format_system_time)
                .unwrap_or_else(|_| "Unavailable".to_string());
            let modified_at = metadata
                .modified()
                .map(format_system_time)
                .unwrap_or_else(|_| "Unavailable".to_string());

            let path_clone = path.clone();
            let config_clone = self.config.clone();
            let db_path_clone = self.db_path.clone();

            let (tx, rx) = mpsc::channel();
            self.is_indexing = true;
            self.indexing_rx = Some(rx);
            self.status_message = format!("Indexing directory: {}", path.display());

            std::thread::spawn(move || {
                let (total_files, indexed_files, unindexed_types, unknown_types) =
                    index_directory_recursive(&path_clone, &config_clone, &db_path_clone, &tx);

                let _ = tx.send(IndexProgress::Finished {
                    total_files,
                    indexed_files,
                    unindexed_types,
                    unknown_types,
                });
            });

            let dir_info = FileMetadataInfo {
                path: path.clone(),
                name,
                extension: "".into(),
                size_bytes: 0,
                file_type: "directory".into(),
                created_at,
                modified_at,
                extra_details: vec![("Status".into(), "Indexing in background...".into())],
                file_hash: format!("dir_{}", path.to_string_lossy()),
                cached_at: format_system_time(SystemTime::now()),
                is_directory: true,
                tags: vec!["directory".into()],
            };

            self.tag_input_buffer = dir_info.tags.join(", ");
            self.current_file = Some(dir_info);
            return;
        }

        let file_hash = match sha256::try_digest(path.as_path()) {
            Ok(h) => h,
            Err(e) => {
                self.status_message = format!("Error computing file hash: {}", e);
                return;
            }
        };

        if let Some(mut cached_info) = self.get_from_db(&file_hash) {
            if let Some(pos) = cached_info.tags.iter().position(|t| t == "scanned") {
                cached_info.tags[pos] = "parsed".to_string();
                self.save_to_db(&cached_info);
            }
            self.tag_input_buffer = cached_info.tags.join(", ");
            self.current_file = Some(cached_info);
            self.status_message = "Loaded file details instantly from SQLite database.".to_string();
            return;
        }

        let name = file_name;
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let size_bytes = metadata.len();
        let file_type = if extension.is_empty() {
            "unknown / binary".to_string()
        } else {
            format!("{} file", extension).to_lowercase()
        };

        let mut initial_tags = vec![file_type.clone(), "parsed".to_string()];
        if let Ok(modified_time) = metadata.modified() {
            let datetime: chrono::DateTime<chrono::Local> = modified_time.into();
            initial_tags.push(datetime.format("%Y").to_string().to_lowercase());
            initial_tags.push(datetime.format("%B").to_string().to_lowercase());
        }

        let created_at = metadata
            .created()
            .map(format_system_time)
            .unwrap_or_else(|_| "Unavailable".to_string());
        let modified_at = metadata
            .modified()
            .map(format_system_time)
            .unwrap_or_else(|_| "Unavailable".to_string());

        let mut extra_details = Vec::new();

        if self.config.included_files.contains(&name) {
            extra_details.push((
                "Config Status".into(),
                "Explicitly Included Priority File".into(),
            ));
        }

        if self
            .config
            .search_criteria
            .image_extensions
            .contains(&extension)
        {
            if let Ok(dims) = image::image_dimensions(&path) {
                extra_details.push(("Width".into(), format!("{} px", dims.0)));
                extra_details.push(("Height".into(), format!("{} px", dims.1)));
                extra_details.push((
                    "Aspect Ratio".into(),
                    format!("{:.2}", dims.0 as f32 / dims.1 as f32),
                ));
            }
        }

        if self
            .config
            .search_criteria
            .media_extensions
            .contains(&extension)
        {
            extra_details.extend(get_media_info_via_ffprobe(&path));
        }

        if self
            .config
            .search_criteria
            .text_extensions
            .contains(&extension)
        {
            if let Ok(text_stats) =
                analyze_text_file(&path, &extension, &self.config.language_extensions)
            {
                extra_details.push(("Language Type".into(), text_stats.language));
                extra_details.push(("Lines of Code (LOC)".into(), text_stats.loc.to_string()));
                extra_details.push(("Total Characters".into(), text_stats.chars.to_string()));
            }
        }

        let file_info = FileMetadataInfo {
            path: path.clone(),
            name,
            extension,
            size_bytes,
            file_type,
            created_at,
            modified_at,
            extra_details,
            file_hash: file_hash.clone(),
            cached_at: format_system_time(SystemTime::now()),
            is_directory: false,
            tags: initial_tags,
        };

        self.tag_input_buffer = file_info.tags.join(", ");
        self.save_to_db(&file_info);
        self.current_file = Some(file_info);
        self.status_message = "File indexed and stored in SQLite successfully.".to_string();
    }

    fn save_current_tags(&mut self) {
        if let Some(file_info) = &mut self.current_file {
            let parsed_tags: Vec<String> = self
                .tag_input_buffer
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();

            file_info.tags = parsed_tags;

            if file_info.is_directory {
                self.status_message =
                    "Tags updated in current view (Directories are not cached by hash)."
                        .to_string();
            } else {
                let info_clone = file_info.clone();
                self.save_to_db(&info_clone);
                self.status_message =
                    "Tags updated and saved to SQLite database successfully!".to_string();
            }
        }
    }

    fn update_current_file_metadata(&mut self) {
        if let Some(file_info) = &mut self.current_file {
            if file_info.is_directory {
                self.status_message =
                    "Metadata updates are currently only supported for files.".to_string();
                return;
            }

            let path = file_info.path.clone();
            let extension = file_info.extension.clone();

            let file_type = if extension.is_empty() {
                "unknown / binary".to_string()
            } else {
                format!("{} file", extension).to_lowercase()
            };
            file_info.file_type = file_type;

            for tag in &mut file_info.tags {
                *tag = tag.to_lowercase();
            }
            if let Some(pos) = file_info.tags.iter().position(|t| t == "scanned") {
                file_info.tags[pos] = "parsed".to_string();
            } else if !file_info.tags.contains(&"parsed".to_string()) {
                file_info.tags.push("parsed".to_string());
            }

            let mut extra_details = Vec::new();

            if self.config.included_files.contains(&file_info.name) {
                extra_details.push((
                    "Config Status".into(),
                    "Explicitly Included Priority File".into(),
                ));
            }

            if self
                .config
                .search_criteria
                .image_extensions
                .contains(&extension)
            {
                if let Ok(dims) = image::image_dimensions(&path) {
                    extra_details.push(("Width".into(), format!("{} px", dims.0)));
                    extra_details.push(("Height".into(), format!("{} px", dims.1)));
                    extra_details.push((
                        "Aspect Ratio".into(),
                        format!("{:.2}", dims.0 as f32 / dims.1 as f32),
                    ));
                }
            }

            if self
                .config
                .search_criteria
                .media_extensions
                .contains(&extension)
            {
                extra_details.extend(get_media_info_via_ffprobe(&path));
            }

            if self
                .config
                .search_criteria
                .text_extensions
                .contains(&extension)
            {
                if let Ok(text_stats) =
                    analyze_text_file(&path, &extension, &self.config.language_extensions)
                {
                    extra_details.push(("Language Type".into(), text_stats.language));
                    extra_details.push(("Lines of Code (LOC)".into(), text_stats.loc.to_string()));
                    extra_details.push(("Total Characters".into(), text_stats.chars.to_string()));
                }
            }

            file_info.extra_details = extra_details;
            let info_clone = file_info.clone();
            self.save_to_db(&info_clone);
            self.status_message =
                "Metadata re-extracted and updated in SQLite database successfully!".to_string();
        }
    }
}

fn is_file_excluded(file_name: &str, extension: &str, excluded_patterns: &[String]) -> bool {
    for pattern in excluded_patterns {
        if pattern.starts_with("*.") {
            let ext_target = &pattern[2..].to_lowercase();
            if extension == ext_target {
                return true;
            }
        } else if pattern.starts_with('*') && pattern.ends_with('*') {
            let sub = &pattern[1..pattern.len() - 1];
            if file_name.contains(sub) {
                return true;
            }
        } else if pattern.starts_with('*') {
            let suf = &pattern[1..];
            if file_name.ends_with(suf) {
                return true;
            }
        } else if pattern.ends_with('*') {
            let pref = &pattern[0..pattern.len() - 1];
            if file_name.starts_with(pref) {
                return true;
            }
        } else if file_name == pattern {
            return true;
        }
    }
    false
}

fn index_directory_recursive(
    dir_path: &Path,
    config: &AppConfig,
    db_path: &Path,
    tx: &Sender<IndexProgress>,
) -> (usize, usize, HashMap<String, usize>, HashMap<String, usize>) {
    let mut total_files = 0;
    let mut indexed_files = 0;
    let mut unindexed_types: HashMap<String, usize> = HashMap::new();
    let mut unknown_types: HashMap<String, usize> = HashMap::new();

    let conn = Connection::open(db_path).ok();

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().into_owned();

            if entry_path.is_dir() {
                if config.ignored_folders.iter().any(|ig| ig == &file_name) {
                    continue;
                }
                let (sub_total, sub_indexed, sub_unindexed, sub_unknown) =
                    index_directory_recursive(&entry_path, config, db_path, tx);
                total_files += sub_total;
                indexed_files += sub_indexed;
                for (k, v) in sub_unindexed {
                    *unindexed_types.entry(k).or_insert(0) += v;
                }
                for (k, v) in sub_unknown {
                    *unknown_types.entry(k).or_insert(0) += v;
                }
            } else if entry_path.is_file() {
                total_files += 1;

                let extension = entry_path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();

                if is_file_excluded(&file_name, &extension, &config.excluded_files) {
                    *unindexed_types.entry("Excluded File".into()).or_insert(0) += 1;
                    continue;
                }

                let _ = tx.send(IndexProgress::Progress(file_name.clone()));

                if extension.is_empty() {
                    *unknown_types.entry("No Extension".into()).or_insert(0) += 1;
                }

                if let Ok(metadata) = fs::metadata(&entry_path) {
                    if let Ok(file_hash) = sha256::try_digest(entry_path.as_path()) {
                        let mut already_exists = false;
                        if let Some(ref c) = conn {
                            let mut stmt = c
                                .prepare("SELECT 1 FROM files WHERE file_hash = ?")
                                .unwrap();
                            already_exists = stmt.exists(params![file_hash]).unwrap_or(false);
                        }

                        if !already_exists {
                            let size_bytes = metadata.len();
                            let file_type = if extension.is_empty() {
                                "unknown / binary".to_string()
                            } else {
                                format!("{} file", extension).to_lowercase()
                            };

                            let mut initial_tags = vec![file_type.clone(), "scanned".to_string()];
                            if let Ok(modified_time) = metadata.modified() {
                                let datetime: chrono::DateTime<chrono::Local> =
                                    modified_time.into();
                                initial_tags.push(datetime.format("%Y").to_string().to_lowercase());
                                initial_tags.push(datetime.format("%B").to_string().to_lowercase());
                            }

                            let created_at = metadata
                                .created()
                                .map(format_system_time)
                                .unwrap_or_else(|_| "Unavailable".to_string());
                            let modified_at = metadata
                                .modified()
                                .map(format_system_time)
                                .unwrap_or_else(|_| "Unavailable".to_string());

                            let mut extra_details = Vec::new();
                            if config.included_files.contains(&file_name) {
                                extra_details.push((
                                    "Config Status".into(),
                                    "Explicitly Included Priority File".into(),
                                ));
                            }

                            let file_info = FileMetadataInfo {
                                path: entry_path.clone(),
                                name: file_name,
                                extension,
                                size_bytes,
                                file_type,
                                created_at,
                                modified_at,
                                extra_details,
                                file_hash: file_hash.clone(),
                                cached_at: format_system_time(SystemTime::now()),
                                is_directory: false,
                                tags: initial_tags,
                            };

                            if let Some(ref c) = conn {
                                let extra_json = serde_json::to_string(&file_info.extra_details)
                                    .unwrap_or_default();
                                let tags_json =
                                    serde_json::to_string(&file_info.tags).unwrap_or_default();
                                let _ = c.execute(
                                    "INSERT OR REPLACE INTO files (file_hash, path, name, extension, size_bytes, file_type, created_at, modified_at, extra_details, cached_at, is_directory, tags)
                                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                                    params![
                                        file_info.file_hash,
                                        file_info.path.to_string_lossy().to_string(),
                                        file_info.name,
                                        file_info.extension,
                                        file_info.size_bytes as i64,
                                        file_info.file_type,
                                        file_info.created_at,
                                        file_info.modified_at,
                                        extra_json,
                                        file_info.cached_at,
                                        file_info.is_directory,
                                        tags_json
                                    ],
                                );
                            }
                        }
                        indexed_files += 1;
                    } else {
                        *unindexed_types.entry("Hash Error".into()).or_insert(0) += 1;
                    }
                } else {
                    *unindexed_types.entry("Metadata Error".into()).or_insert(0) += 1;
                }
            }
        }
    }

    (total_files, indexed_files, unindexed_types, unknown_types)
}

#[derive(Deserialize)]
struct FFProbeOutput {
    streams: Option<Vec<Stream>>,
    format: Option<Format>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Deserialize)]
struct Format {
    duration: Option<String>,
    bit_rate: Option<String>,
}

fn get_media_info_via_ffprobe(path: &Path) -> Vec<(String, String)> {
    let mut details = Vec::new();
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path.to_str().unwrap_or_default(),
        ])
        .output();

    if let Ok(res) = output {
        if let Ok(info) = serde_json::from_slice::<FFProbeOutput>(&res.stdout) {
            if let Some(streams) = info.streams {
                for stream in streams {
                    if stream.codec_type == "video" {
                        if let (Some(w), Some(h)) = (stream.width, stream.height) {
                            details.push(("Video Resolution".into(), format!("{}x{} px", w, h)));
                        }
                    }
                    if let Some(codec) = stream.codec_name {
                        details.push((
                            format!("{} Codec", stream.codec_type.to_uppercase()),
                            codec.to_uppercase(),
                        ));
                    }
                }
            }

            if let Some(format) = info.format {
                if let Some(dur_str) = format.duration {
                    if let Ok(seconds) = dur_str.parse::<f64>() {
                        let hours = (seconds / 3600.0).floor();
                        let mins = ((seconds % 3600.0) / 60.0).floor();
                        let secs = seconds % 60.0;
                        details.push((
                            "Media Duration".into(),
                            format!("{:02}:{:02}:{:05.2} ({:.1}s)", hours, mins, secs, seconds),
                        ));
                    }
                }
                if let Some(bitrate) = format.bit_rate {
                    if let Ok(bits) = bitrate.parse::<u64>() {
                        details.push(("Bitrate".into(), format!("{} kbps", bits / 1000)));
                    }
                }
            }
        }
    }
    details
}

struct TextStats {
    language: String,
    loc: usize,
    chars: usize,
}

fn analyze_text_file(
    path: &Path,
    ext: &str,
    lang_map: &HashMap<String, String>,
) -> std::io::Result<TextStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut loc = 0;
    let mut chars = 0;

    for line in reader.lines() {
        let l = line?;
        loc += 1;
        chars += l.chars().count();
    }

    let language = lang_map
        .get(ext)
        .cloned()
        .unwrap_or_else(|| "Plain Text".to_string());

    Ok(TextStats {
        language,
        loc,
        chars,
    })
}

fn format_system_time(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

fn open_containing_folder(path: &str) {
    #[cfg(target_os = "windows")]
    {
        let target = path.trim_start_matches(r"\\?\").to_string();

        let _ = std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&target)
            .spawn();
    }
}

impl eframe::App for FileInspectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_pixels_per_point(1.7);

        // Handle indexing background worker progress
        if let Some(rx) = &self.indexing_rx {
            ctx.request_repaint();
            match rx.try_recv() {
                Ok(IndexProgress::Progress(filename)) => {
                    self.current_indexing_file = filename;
                    ctx.request_repaint();
                }
                Ok(IndexProgress::Finished {
                    total_files,
                    indexed_files,
                    unindexed_types,
                    unknown_types,
                }) => {
                    self.is_indexing = false;
                    self.indexing_rx = None;
                    self.status_message = format!(
                        "Indexed {}/{} files successfully into SQLite.",
                        indexed_files, total_files
                    );

                    if let Some(file_info) = &mut self.current_file {
                        let mut extra_details = Vec::new();
                        extra_details
                            .push(("Total Files Encountered".into(), total_files.to_string()));
                        extra_details.push((
                            "Successfully Indexed Files".into(),
                            indexed_files.to_string(),
                        ));

                        let unindexed_summary = if unindexed_types.is_empty() {
                            "None".into()
                        } else {
                            unindexed_types
                                .iter()
                                .map(|(k, v)| format!("{}: {}", k, v))
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        extra_details.push(("Un-indexed Files / Types".into(), unindexed_summary));

                        let unknown_summary = if unknown_types.is_empty() {
                            "None".into()
                        } else {
                            unknown_types
                                .iter()
                                .map(|(k, v)| format!("{}: {}", k, v))
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        extra_details.push(("Unknown File Types".into(), unknown_summary));
                        file_info.extra_details = extra_details;
                    }
                    ctx.request_repaint();
                }
                Err(_) => {}
            }
        }

        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(path) = &file.path {
                        self.inspect_path(path.clone());
                    }
                }
            }
        });

        // 1. RIGHT PANEL: Search Results & Query Controls
        egui::SidePanel::right("search_results_panel")
            .default_width(360.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Search & Explorer");
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui.button("📁 Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.inspect_path(path);
                        }
                    }
                    if ui.button("🔄 Reload").clicked() {
                        self.reload_config();
                    }
                    if ui.button("📤 Export DB").clicked() {
                        let all_items = self.fetch_all_from_db();
                        self.export_to_json(&all_items, "whole_database");
                    }
                });

                ui.add_space(4.0);
                ui.label(&self.status_message);

                if self.is_indexing {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("Processing: {}", self.current_indexing_file))
                            .color(egui::Color32::YELLOW),
                    );
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("🔍 Query:");
                    ui.add(egui::TextEdit::singleline(&mut self.search_query).desired_width(190.0));
                    if !self.search_query.is_empty() && ui.button("Clear").clicked() {
                        self.search_query.clear();
                    }
                });

                ui.add_space(6.0);
                ui.strong("Search Results");
                ui.add_space(4.0);

                let all_files = self.fetch_all_from_db();
                let terms: Vec<String> = self
                    .search_query
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();

                let matches: Vec<FileMetadataInfo> = all_files
                    .iter()
                    .filter(|info| {
                        if terms.is_empty() {
                            return true;
                        }
                        terms.iter().any(|term| {
                            let name_match = info.name.to_lowercase().contains(term);
                            let tag_match =
                                info.tags.iter().any(|t| t.to_lowercase().contains(term));
                            let ext_match = info.extension.to_lowercase() == *term;
                            name_match || tag_match || ext_match
                        })
                    })
                    .cloned()
                    .collect();

                if !matches.is_empty() {
                    if ui.button("📤 Export Current Results").clicked() {
                        self.export_to_json(&matches, "search_results");
                    }
                }

                ui.add_space(4.0);

                egui::ScrollArea::vertical().show(ui, |ui| {
                    if matches.is_empty() {
                        ui.label(
                            egui::RichText::new("No indexed files found matching query.")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                    } else {
                        for file_info in matches {
                            let is_selected = self
                                .current_file
                                .as_ref()
                                .map_or(false, |curr| curr.file_hash == file_info.file_hash);

                            ui.horizontal(|ui| {
                                // File Select
                                if ui.button("👆").clicked() {
                                    let path = file_info.path.clone();
                                    self.inspect_path(path);
                                }
                                let short_name = Self::truncate_name(&file_info.name, 15);
                                let display_str = format!(
                                    "{} [{}, {}]",
                                    short_name,
                                    file_info.file_type,
                                    file_info.tags.join(", ")
                                );

                                let mut text = egui::RichText::new(display_str);
                                if is_selected {
                                    text = text.color(egui::Color32::LIGHT_BLUE).strong();
                                }

                                ui.label(text);
                            });
                        }
                    }
                });
            });

        // 2. RIGHT PANEL (CentralPanel): General Attributes, Custom Tags Management, Parsed Format Metadata
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(file_info) = &self.current_file {
                    let is_directory = file_info.is_directory;
                    let name = file_info.name.clone();
                    let path_str = file_info.path.to_string_lossy().to_string();
                    let file_hash = file_info.file_hash.clone();
                    let file_type = file_info.file_type.clone();
                    let size_bytes = file_info.size_bytes;
                    let created_at = file_info.created_at.clone();
                    let modified_at = file_info.modified_at.clone();
                    let tags = file_info.tags.clone();
                    let extra_details = file_info.extra_details.clone();

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.strong(if is_directory { "Directory Attributes" } else { "General Attributes" });
                        ui.add_space(4.0);

                        egui::Grid::new("file_meta_grid")
                            .num_columns(2)
                            .spacing([40.0, 6.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.label("Name");
                                ui.add(egui::Label::new(&name).wrap());
                                ui.end_row();

                                ui.label("Full Path");
                                ui.horizontal(|ui| {
                                    if ui.button("🔍").clicked() {
                                        open_containing_folder(&path_str);
                                    }
                                    ui.add(egui::Label::new(path_str).wrap().sense(egui::Sense::hover()));
                                });
                                ui.end_row();

                                if !is_directory {
                                    ui.label("SHA-256 Hash");
                                    ui.add(egui::Label::new(&file_hash).wrap());
                                    ui.end_row();
                                }

                                ui.label("Type");
                                ui.label(&file_type);
                                ui.end_row();

                                if !is_directory {
                                    ui.label("Size");
                                    ui.label(format!("{} ({} bytes)", format_file_size(size_bytes), size_bytes));
                                    ui.end_row();
                                }

                                ui.label("Created At");
                                ui.label(&created_at);
                                ui.end_row();

                                ui.label("Last Modified");
                                ui.label(&modified_at);
                                ui.end_row();
                            });
                    });

                    ui.add_space(10.0);

                    ui.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.strong("Custom Tags Management");
                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label("Tags:");
                            ui.add(egui::TextEdit::singleline(&mut self.tag_input_buffer).desired_width(260.0));
                            if ui.button("💾 Save Tags").clicked() {
                                self.save_current_tags();
                            }
                            if ui.button("🔄 Update Metadata").clicked() {
                                self.update_current_file_metadata();
                            }
                        });

                        if !tags.is_empty() {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label("Current Tags:");
                                for tag in &tags {
                                    ui.add(egui::Label::new(format!("[{}]", tag)));
                                }
                            });
                        }
                    });

                    ui.add_space(10.0);

                    if !extra_details.is_empty() {
                        ui.group(|ui| {
                            ui.set_width(ui.available_width());
                            ui.strong("Parsed Format Metadata");
                            ui.add_space(4.0);

                            egui::Grid::new("extra_meta_grid")
                                .num_columns(2)
                                .spacing([40.0, 6.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    for (key, val) in &extra_details {
                                        ui.label(key);
                                        ui.add(egui::Label::new(val).wrap());
                                        ui.end_row();
                                    }
                                });
                        });
                    }
                } else {
                    ui.add_space(120.0);
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new("Drag & Drop a file or folder anywhere onto this window,\nor click 'Browse...' in the left panel to inspect.")
                                .color(egui::Color32::GRAY)
                                .italics(),
                        );
                    });
                }
            });
        });
    }
}
