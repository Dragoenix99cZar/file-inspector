use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
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
        let mut lang_exts = HashMap::new();
        lang_exts.insert("rs".to_string(), "Rust".to_string());
        lang_exts.insert("py".to_string(), "Python".to_string());
        lang_exts.insert("js".to_string(), "JavaScript".to_string());
        lang_exts.insert("ts".to_string(), "TypeScript".to_string());
        lang_exts.insert("md".to_string(), "Markdown".to_string());
        lang_exts.insert("toml".to_string(), "TOML Config".to_string());
        lang_exts.insert("json".to_string(), "JSON Data".to_string());
        lang_exts.insert("yaml".to_string(), "YAML Config".to_string());
        lang_exts.insert("yml".to_string(), "YAML Config".to_string());
        lang_exts.insert("xml".to_string(), "XML Document".to_string());
        lang_exts.insert("html".to_string(), "HTML Markup".to_string());
        lang_exts.insert("css".to_string(), "Cascading Style Sheets".to_string());
        lang_exts.insert("c".to_string(), "C Source".to_string());
        lang_exts.insert("cpp".to_string(), "C++ Source".to_string());
        lang_exts.insert("h".to_string(), "C/C++ Header".to_string());

        Self {
            language_extensions: lang_exts,
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
            excluded_files: vec![".DS_Store".into(), "Thumbs.db".into(), "desktop.ini".into()],
            ignored_folders: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                ".vs".into(),
                ".idea".into(),
                "dist".into(),
                "build".into(),
            ],
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([850.0, 720.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Advanced File Inspector - egui",
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

struct FileInspectorApp {
    current_file: Option<FileMetadataInfo>,
    status_message: String,
    json_db_path: PathBuf,
    config_path: PathBuf,
    file_hash_map: HashMap<String, String>,
    config: AppConfig,
    tag_input_buffer: String,
}

impl Default for FileInspectorApp {
    fn default() -> Self {
        let exe_path = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));
        let json_db_path = exe_dir.join("file_index.json");
        let config_path = exe_dir.join("config.json");

        let config = if config_path.exists() {
            fs::read_to_string(&config_path)
                .ok()
                .and_then(|c| serde_json::from_str(&c).ok())
                .unwrap_or_else(AppConfig::default)
        } else {
            let default_cfg = AppConfig::default();
            if let Ok(serialized) = serde_json::to_string_pretty(&default_cfg) {
                let _ = fs::write(&config_path, serialized);
            }
            default_cfg
        };

        let file_hash_map = load_hash_map_from_json(&json_db_path);

        Self {
            current_file: None,
            status_message: format!("Loaded config from: {}", config_path.display()),
            json_db_path,
            config_path,
            file_hash_map,
            config,
            tag_input_buffer: String::new(),
        }
    }
}

impl FileInspectorApp {
    fn reload_config(&mut self) {
        if self.config_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.config_path) {
                if let Ok(parsed) = serde_json::from_str::<AppConfig>(&content) {
                    self.config = parsed;
                    self.status_message =
                        "Configuration reloaded successfully from config.json.".to_string();
                    return;
                }
            }
        }
        self.status_message =
            "Failed to reload config.json (syntax error or missing file).".to_string();
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

        if !metadata.is_dir() && self.config.excluded_files.contains(&file_name) {
            self.status_message = format!(
                "Skipped: '{}' is explicitly excluded in config.json.",
                file_name
            );
            return;
        }

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

            let (total_size, file_count) = count_dir_contents(
                &path,
                &self.config.ignored_folders,
                &self.config.excluded_files,
            );

            let mut extra_details = Vec::new();
            extra_details.push(("Total Files & Folders".into(), file_count.to_string()));

            let dir_info = FileMetadataInfo {
                path: path.clone(),
                name,
                extension: "".into(),
                size_bytes: total_size,
                file_type: "Directory".into(),
                created_at,
                modified_at,
                extra_details,
                file_hash: "N/A (Directory)".into(),
                cached_at: format_system_time(SystemTime::now()),
                is_directory: true,
                tags: vec![],
            };

            self.tag_input_buffer.clear();
            self.current_file = Some(dir_info);
            self.status_message =
                "Directory inspected successfully (config rules applied).".to_string();
            return;
        }

        let file_hash = match sha256::try_digest(path.as_path()) {
            Ok(h) => h,
            Err(e) => {
                self.status_message = format!("Error computing file hash: {}", e);
                return;
            }
        };

        if let Some(cached_info) = self.load_cached_metadata(&file_hash) {
            self.tag_input_buffer = cached_info.tags.join(", ");
            self.current_file = Some(cached_info);
            self.status_message =
                "Loaded file details instantly from local JSON cache (Hash match).".to_string();
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
            "Unknown / Binary".to_string()
        } else {
            format!("{} file", extension.to_uppercase())
        };

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

        let cached_at = format_system_time(SystemTime::now());

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
            cached_at,
            is_directory: false,
            tags: vec![],
        };

        self.tag_input_buffer.clear();
        self.file_hash_map
            .insert(file_hash.clone(), path.to_string_lossy().into_owned());
        self.save_metadata_to_cache(&file_hash, &file_info);

        self.current_file = Some(file_info);
        self.status_message =
            "File inspected and recorded to local JSON cache successfully.".to_string();
    }

    fn save_current_tags(&mut self) {
        if let Some(file_info) = &mut self.current_file {
            let parsed_tags: Vec<String> = self
                .tag_input_buffer
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            file_info.tags = parsed_tags;

            if file_info.is_directory {
                self.status_message =
                    "Tags updated in current view (Directories are not cached by hash)."
                        .to_string();
            } else {
                let hash = file_info.file_hash.clone();
                let info_clone = file_info.clone();
                self.save_metadata_to_cache(&hash, &info_clone);
                self.status_message =
                    "Tags successfully updated and saved to file_index.json!".to_string();
            }
        }
    }

    fn load_cached_metadata(&self, hash: &str) -> Option<FileMetadataInfo> {
        if self.file_hash_map.contains_key(hash) {
            if let Ok(file_content) = fs::read_to_string(&self.json_db_path) {
                if let Ok(full_cache) =
                    serde_json::from_str::<HashMap<String, FileMetadataInfo>>(&file_content)
                {
                    return full_cache.get(hash).cloned();
                }
            }
        }
        None
    }

    fn save_metadata_to_cache(&self, hash: &str, info: &FileMetadataInfo) {
        let mut full_cache: HashMap<String, FileMetadataInfo> = if self.json_db_path.exists() {
            fs::read_to_string(&self.json_db_path)
                .ok()
                .and_then(|content| serde_json::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        full_cache.insert(hash.to_string(), info.clone());

        if let Ok(serialized) = serde_json::to_string_pretty(&full_cache) {
            let _ = fs::write(&self.json_db_path, serialized);
        }
    }
}

fn count_dir_contents(
    path: &Path,
    ignored_folders: &[String],
    excluded_files: &[String],
) -> (u64, usize) {
    let mut total_size = 0;
    let mut file_count = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            let file_name = entry.file_name().to_string_lossy().into_owned();

            if ignored_folders.iter().any(|ig| ig == &file_name)
                || excluded_files.iter().any(|ex| ex == &file_name)
            {
                continue;
            }

            file_count += 1;
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    let (sub_size, sub_count) =
                        count_dir_contents(&entry_path, ignored_folders, excluded_files);
                    total_size += sub_size;
                    file_count += sub_count;
                } else {
                    total_size += meta.len();
                }
            }
        }
    }
    (total_size, file_count)
}

fn load_hash_map_from_json(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(full_cache) =
                serde_json::from_str::<HashMap<String, FileMetadataInfo>>(&content)
            {
                for (hash, info) in full_cache {
                    map.insert(hash, info.path.to_string_lossy().into_owned());
                }
            }
        }
    }
    map
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

impl eframe::App for FileInspectorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if let Some(file) = i.raw.dropped_files.first() {
                    if let Some(path) = &file.path {
                        self.inspect_path(path.clone());
                    }
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Windows 11 File Inspector");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("📁 Browse...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.inspect_path(path);
                    }
                }
                if ui.button("🔄 Reload Config").clicked() {
                    self.reload_config();
                }
                ui.label(&self.status_message);
            });

            ui.separator();
            ui.add_space(4.0);

            // Extract values upfront if current_file is present to avoid borrowing `self` immutably for too long
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
                            ui.add(egui::Label::new(path_str).wrap().sense(egui::Sense::hover()));
                            ui.end_row();

                            if !is_directory {
                                ui.label("SHA-256 Hash");
                                ui.add(egui::Label::new(&file_hash).wrap());
                                ui.end_row();
                            }

                            ui.label("Type");
                            ui.label(&file_type);
                            ui.end_row();

                            ui.label("Size");
                            ui.label(format!("{} ({} bytes)", format_file_size(size_bytes), size_bytes));
                            ui.end_row();

                            ui.label("Created At");
                            ui.label(&created_at);
                            ui.end_row();

                            ui.label("Last Modified");
                            ui.label(&modified_at);
                            ui.end_row();
                        });
                });

                ui.add_space(10.0);

                // Custom Tag Management Section (Now safely able to mutate `self` via `self.save_current_tags()`)
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.strong("Custom Tags Management");
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Tags (comma separated):");
                        ui.add(egui::TextEdit::singleline(&mut self.tag_input_buffer).desired_width(350.0));
                        if ui.button("💾 Save Tags").clicked() {
                            self.save_current_tags();
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
                ui.add_space(40.0);
                ui.centered_and_justified(|ui| {
                    ui.label(
                        egui::RichText::new("Drag & Drop a file or folder anywhere onto this window\nor click 'Browse...' to inspect.")
                            .color(egui::Color32::GRAY)
                            .italics(),
                    );
                });
            }
        });
    }
}
