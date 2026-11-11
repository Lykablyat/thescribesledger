use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use eframe::egui;
use crate::compile::compile_book_to_pdf;
use crate::compiler::compile_to_pdf;
use crate::project::{BookProject, ChapterKind};
use crate::scrivener::ScrivenerImporter;

/// Font family choices available in the format bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFamilyChoice {
    Serif,
    Proportional,
    Monospace,
}

impl FontFamilyChoice {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Serif => "Sitka Text",
            Self::Proportional => "Segoe UI",
            Self::Monospace => "Consolas",
        }
    }

    pub fn to_egui_family(&self) -> egui::FontFamily {
        match self {
            Self::Serif => egui::FontFamily::Name("Serif".into()),
            Self::Proportional => egui::FontFamily::Proportional,
            Self::Monospace => egui::FontFamily::Monospace,
        }
    }
}

/// Identifies which document is currently active in the central editor pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedDocument {
    FrontMatter(usize),
    Scene { chapter_idx: usize, scene_idx: usize },
}

/// Target for the writer-only rename modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameTarget {
    Chapter(usize),
    Scene { chapter_idx: usize, scene_idx: usize },
}

/// Target for the delete confirmation modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Chapter(usize),
    Scene { chapter_idx: usize, scene_idx: usize },
}

/// Deferred sidebar actions executed after tree rendering.
enum SidebarAction {
    SelectFrontMatter(usize),
    SelectScene(usize, usize),
    StartRenameChapter(usize, String),
    StartRenameScene(usize, usize, String),
    ConfirmDeleteChapter(usize),
    ConfirmDeleteScene(usize, usize),
    MoveChapter { from: usize, to: usize },
    MoveScene { from_ch: usize, from_sc: usize, to_ch: usize, to_sc: usize },
}

/// Drag-and-drop payload for reordering chapters and scenes in the binder tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinderDragPayload {
    Chapter(usize),
    Scene { chapter_idx: usize, scene_idx: usize },
}

// The Scribe's Ledger Monochromatic Deep Green Palette Constants
const SCRIV_TOOLBAR_BG: egui::Color32 = egui::Color32::from_rgb(246, 248, 246);
const SCRIV_FORMAT_BAR_BG: egui::Color32 = egui::Color32::from_rgb(248, 250, 248);
const SCRIV_HEADER_BAR_BG: egui::Color32 = egui::Color32::from_rgb(243, 246, 244);
const SCRIV_BINDER_BG: egui::Color32 = egui::Color32::from_rgb(240, 244, 241);
const SCRIV_BINDER_SELECTED_BG: egui::Color32 = egui::Color32::from_rgb(214, 230, 218);
const SCRIV_CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(250, 252, 250);
const SCRIV_PAPER_BG: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);
const SCRIV_BORDER: egui::Color32 = egui::Color32::from_rgb(218, 226, 220);
const SCRIV_FOLDER_BLUE: egui::Color32 = egui::Color32::from_rgb(26, 88, 52); // Deep rich bookbinder green
const SCRIV_SPECIAL_GOLD: egui::Color32 = egui::Color32::from_rgb(38, 114, 68); // Deep emerald accent
const SCRIV_TEXT_DARK: egui::Color32 = egui::Color32::from_rgb(30, 36, 32);
const SCRIV_TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(95, 112, 100);

// The Scribe's Ledger Semantic Palette Aliases
pub const LEDGER_TOOLBAR_BG: egui::Color32 = SCRIV_TOOLBAR_BG;
pub const LEDGER_FORMAT_BAR_BG: egui::Color32 = SCRIV_FORMAT_BAR_BG;
pub const LEDGER_HEADER_BAR_BG: egui::Color32 = SCRIV_HEADER_BAR_BG;
pub const LEDGER_BINDER_BG: egui::Color32 = SCRIV_BINDER_BG;
pub const LEDGER_BINDER_SELECTED_BG: egui::Color32 = SCRIV_BINDER_SELECTED_BG;
pub const LEDGER_CANVAS_BG: egui::Color32 = SCRIV_CANVAS_BG;
pub const LEDGER_PAPER_BG: egui::Color32 = SCRIV_PAPER_BG;
pub const LEDGER_BORDER: egui::Color32 = SCRIV_BORDER;
pub const LEDGER_GREEN_PRIMARY: egui::Color32 = SCRIV_FOLDER_BLUE;
pub const LEDGER_GREEN_ACCENT: egui::Color32 = SCRIV_SPECIAL_GOLD;
pub const LEDGER_TEXT_DARK: egui::Color32 = SCRIV_TEXT_DARK;
pub const LEDGER_TEXT_MUTED: egui::Color32 = SCRIV_TEXT_MUTED;

/// The main application state for The Scribe's Ledger.
pub struct BookApp {
    pub project: Option<BookProject>,
    pub selected_doc: Option<SelectedDocument>,
    pub editor_content: String,
    pub cached_word_count: usize,
    pub is_dirty: bool,
    pub current_file_path: Option<PathBuf>,
    pub last_edit_time: Option<Instant>,

    // Navigation history (Scrivener `<` `>` header buttons)
    pub nav_history: Vec<SelectedDocument>,
    pub nav_history_pos: usize,

    // Layout & Sidebar state
    pub sidebar_collapsed: bool,
    pub sidebar_width: f32,
    pub zoom_percent: u32,
    pub line_spacing: f32,

    // Toolbar settings
    pub font_family: FontFamilyChoice,
    pub font_size: f32,
    pub is_bold: bool,
    pub is_italic: bool,
    pub is_underline: bool,

    // Status notifications
    pub status_message: Option<(String, Instant)>,

    // Modal dialogs
    pub show_new_blueprint_modal: bool,
    pub new_title_input: String,
    pub new_author_input: String,

    // Project metadata modal (edit Book Title & Author)
    pub show_project_metadata_modal: bool,
    pub edit_project_title_input: String,
    pub edit_project_author_input: String,

    // Scrivener (.scriv) import modal
    pub show_scrivener_import_modal: bool,
    pub scriv_source_path: Option<PathBuf>,
    pub scriv_dest_path: Option<PathBuf>,
    pub scriv_title_input: String,
    pub scriv_author_input: String,

    // Specialized chapter modal
    pub show_specialized_modal: bool,
    pub selected_specialized_preset: String,
    pub custom_specialized_title: String,
    pub use_custom_specialized_title: bool,
    pub insert_specialized_at_start: bool,

    // Rename modal (writer-only view names)
    pub rename_target: Option<RenameTarget>,
    pub rename_input: String,

    // Delete confirmation modal
    pub delete_target: Option<DeleteTarget>,

    // Editor formatting display toggle
    pub markdown_in_editor: bool,
}

impl Default for BookApp {
    fn default() -> Self {
        Self {
            project: None,
            selected_doc: None,
            editor_content: String::new(),
            cached_word_count: 0,
            is_dirty: false,
            current_file_path: None,
            last_edit_time: None,
            nav_history: Vec::new(),
            nav_history_pos: 0,
            sidebar_collapsed: false,
            sidebar_width: 245.0,
            zoom_percent: 100,
            line_spacing: 1.0,
            font_family: FontFamilyChoice::Serif,
            font_size: 15.0,
            is_bold: false,
            is_italic: false,
            is_underline: false,
            status_message: None,
            show_new_blueprint_modal: false,
            new_title_input: "My Novel".to_string(),
            new_author_input: "Author Name".to_string(),
            show_project_metadata_modal: false,
            edit_project_title_input: String::new(),
            edit_project_author_input: String::new(),
            show_scrivener_import_modal: false,
            scriv_source_path: None,
            scriv_dest_path: None,
            scriv_title_input: String::new(),
            scriv_author_input: String::new(),
            show_specialized_modal: false,
            selected_specialized_preset: "Prologue".to_string(),
            custom_specialized_title: String::new(),
            use_custom_specialized_title: false,
            insert_specialized_at_start: true,
            rename_target: None,
            rename_input: String::new(),
            delete_target: None,
            markdown_in_editor: true,
        }
    }
}

impl BookApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_scrivener_theme_and_fonts(&cc.egui_ctx);

        let mut app = Self::default();

        // Check if an existing project is in current directory or a default location
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let default_novel_path = current_dir.join("novel_project");

        if current_dir.join("book.toml").exists() {
            let _ = app.load_project(&current_dir);
        } else if default_novel_path.join("book.toml").exists() {
            let _ = app.load_project(&default_novel_path);
        } else if let Ok(proj) =
            BookProject::create_novel_blueprint(&default_novel_path, "The Silent Echo", "Jane Doe")
        {
            app.set_project(proj);
        }

        app
    }

    #[inline]
    fn recompute_word_count(&mut self) {
        self.cached_word_count = self.editor_content.split_whitespace().count();
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    pub fn open_project_metadata_modal(&mut self) {
        if let Some(p) = &self.project {
            self.edit_project_title_input = p.title.clone();
            self.edit_project_author_input = p.author.clone();
            self.show_project_metadata_modal = true;
        } else {
            self.set_status("No project loaded to edit metadata");
        }
    }

    /// Configures and opens the Scrivener Import Modal, optionally pre-populating from a selected `.scriv` path.
    pub fn open_scrivener_import_modal(&mut self, source: Option<PathBuf>) {
        self.save_current_file_if_dirty();
        if let Some(src) = source {
            self.apply_scrivener_source_selection(src);
        }
        self.show_scrivener_import_modal = true;
    }

    fn apply_scrivener_source_selection(&mut self, src: PathBuf) {
        if let Ok((bundle_dir, scrivx_path)) = ScrivenerImporter::resolve_scriv_paths(&src) {
            let inferred_title = scrivx_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported Novel")
                .to_string();
            self.scriv_title_input = inferred_title.clone();
            if self.scriv_author_input.trim().is_empty() {
                self.scriv_author_input = "Author".to_string();
            }
            if self.scriv_dest_path.is_none() {
                if let Some(parent) = bundle_dir.parent() {
                    let safe_folder = format!("{}_ScribesLedger", sanitize_filename(&inferred_title));
                    self.scriv_dest_path = Some(parent.join(safe_folder));
                }
            }
            self.scriv_source_path = Some(bundle_dir);
        } else {
            self.scriv_source_path = Some(src);
        }
    }

    /// Runs the Scrivener import engine and switches the active project to the newly ported novel.
    pub fn import_scrivener_project(
        &mut self,
        scriv_source: &Path,
        dest_dir: &Path,
        override_title: Option<&str>,
        override_author: Option<&str>,
    ) -> anyhow::Result<()> {
        self.save_current_file_if_dirty();
        let report = ScrivenerImporter::import(scriv_source, dest_dir, override_title, override_author)?;
        let ch_count = report.chapters_imported;
        let sc_count = report.scenes_imported;
        let title = report.project.title.clone();
        self.set_project(report.project);
        self.set_status(format!(
            "Imported Scrivener project '{}' ({} chapters, {} scenes)",
            title, ch_count, sc_count
        ));
        Ok(())
    }

    pub fn set_project(&mut self, project: BookProject) {
        self.save_current_file_if_dirty();
        self.project = Some(project);
        self.nav_history.clear();
        self.nav_history_pos = 0;

        if let Some(p) = &self.project {
            if !p.chapters.is_empty() && !p.chapters[0].scenes.is_empty() {
                self.select_scene(0, 0);
            } else if !p.front_matter.is_empty() {
                self.select_front_matter(0);
            } else {
                self.selected_doc = None;
                self.current_file_path = None;
                self.editor_content.clear();
                self.cached_word_count = 0;
                self.is_dirty = false;
            }
        }
        self.set_status("Project loaded");
    }

    pub fn load_project<P: AsRef<Path>>(&mut self, path: P) -> anyhow::Result<()> {
        let proj = BookProject::open(path)?;
        self.set_project(proj);
        Ok(())
    }

    fn record_nav(&mut self, doc: SelectedDocument) {
        if self.nav_history.get(self.nav_history_pos) == Some(&doc) {
            return;
        }
        if self.nav_history_pos + 1 < self.nav_history.len() {
            self.nav_history.truncate(self.nav_history_pos + 1);
        }
        self.nav_history.push(doc);
        self.nav_history_pos = self.nav_history.len() - 1;
    }

    pub fn navigate_back(&mut self) {
        if self.nav_history_pos > 0 {
            self.nav_history_pos -= 1;
            if let Some(doc) = self.nav_history.get(self.nav_history_pos).cloned() {
                self.load_selected_doc_without_history(doc);
            }
        }
    }

    pub fn navigate_forward(&mut self) {
        if self.nav_history_pos + 1 < self.nav_history.len() {
            self.nav_history_pos += 1;
            if let Some(doc) = self.nav_history.get(self.nav_history_pos).cloned() {
                self.load_selected_doc_without_history(doc);
            }
        }
    }

    fn load_selected_doc_without_history(&mut self, doc: SelectedDocument) {
        match doc {
            SelectedDocument::FrontMatter(idx) => self.select_front_matter_inner(idx, false),
            SelectedDocument::Scene {
                chapter_idx,
                scene_idx,
            } => self.select_scene_inner(chapter_idx, scene_idx, false),
        }
    }

    /// Selects and loads a front matter document directly from disk, auto-saving any previous edits.
    pub fn select_front_matter(&mut self, idx: usize) {
        self.select_front_matter_inner(idx, true);
    }

    fn select_front_matter_inner(&mut self, idx: usize, push_history: bool) {
        self.save_current_file_if_dirty();
        let item_path = self
            .project
            .as_ref()
            .and_then(|p| p.front_matter.get(idx).map(|item| item.path.clone()));
        if let Some(path) = item_path {
            let content = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    self.set_status(format!("Error reading {:?}: {}", path, e));
                    String::new()
                }
            };
            self.editor_content = content;
            self.recompute_word_count();
            self.current_file_path = Some(path);
            let doc = SelectedDocument::FrontMatter(idx);
            self.selected_doc = Some(doc.clone());
            if push_history {
                self.record_nav(doc);
            }
            self.is_dirty = false;
            self.last_edit_time = None;
        }
    }

    /// Selects and loads a scene file directly from disk, auto-saving any previous edits.
    pub fn select_scene(&mut self, chapter_idx: usize, scene_idx: usize) {
        self.select_scene_inner(chapter_idx, scene_idx, true);
    }

    fn select_scene_inner(&mut self, chapter_idx: usize, scene_idx: usize, push_history: bool) {
        self.save_current_file_if_dirty();
        let scene_path = self.project.as_ref().and_then(|p| {
            p.chapters
                .get(chapter_idx)
                .and_then(|ch| ch.scenes.get(scene_idx).map(|sc| sc.path.clone()))
        });
        if let Some(path) = scene_path {
            let content = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    self.set_status(format!("Error reading {:?}: {}", path, e));
                    String::new()
                }
            };
            self.editor_content = content;
            self.recompute_word_count();
            self.current_file_path = Some(path);
            let doc = SelectedDocument::Scene {
                chapter_idx,
                scene_idx,
            };
            self.selected_doc = Some(doc.clone());
            if push_history {
                self.record_nav(doc);
            }
            self.is_dirty = false;
            self.last_edit_time = None;
        }
    }

    /// Automatically writes changes to disk if marked dirty.
    pub fn save_current_file_if_dirty(&mut self) {
        if self.is_dirty {
            self.save_current_file();
        }
    }

    /// Saves the current editor content to its active file on disk.
    pub fn save_current_file(&mut self) {
        if let Some(path) = &self.current_file_path {
            if let Err(e) = std::fs::write(path, &self.editor_content) {
                self.set_status(format!("Error saving file: {}", e));
            } else {
                self.is_dirty = false;
                self.last_edit_time = None;
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("document");
                self.set_status(format!("Saved {}", filename));
            }
        }
    }

    pub fn add_chapter(&mut self) {
        self.save_current_file_if_dirty();
        if let Some(p) = &mut self.project {
            match p.create_next_chapter() {
                Ok(new_ch_idx) => {
                    let title = p.chapters[new_ch_idx].title.clone();
                    self.select_scene(new_ch_idx, 0);
                    self.set_status(format!("Added {}", title));
                }
                Err(e) => {
                    self.set_status(format!("Error adding chapter: {}", e));
                }
            }
        }
    }

    pub fn add_specialized_chapter(&mut self, special_title: &str, insert_at_beginning: bool) {
        self.save_current_file_if_dirty();
        if let Some(p) = &mut self.project {
            match p.create_specialized_chapter(special_title, insert_at_beginning) {
                Ok(ch_idx) => {
                    self.select_scene(ch_idx, 0);
                    self.set_status(format!("Added specialized chapter '{}'", special_title));
                }
                Err(e) => {
                    self.set_status(format!("Error adding specialized chapter: {}", e));
                }
            }
        }
    }

    pub fn add_scene(&mut self) {
        self.save_current_file_if_dirty();
        let ch_idx = match self.selected_doc {
            Some(SelectedDocument::Scene { chapter_idx, .. }) => chapter_idx,
            _ => 0,
        };

        let result = if let Some(p) = &mut self.project {
            if p.chapters.is_empty() {
                None
            } else {
                let target_ch = ch_idx.min(p.chapters.len() - 1);
                Some((
                    target_ch,
                    p.create_next_scene(target_ch),
                    p.chapters[target_ch].title.clone(),
                ))
            }
        } else {
            None
        };

        match result {
            None => {
                self.add_chapter();
            }
            Some((target_ch, Ok(new_sc_idx), title)) => {
                self.select_scene(target_ch, new_sc_idx);
                self.set_status(format!("Added Scene {} to {}", new_sc_idx + 1, title));
            }
            Some((_, Err(e), _)) => {
                self.set_status(format!("Error adding scene: {}", e));
            }
        }
    }

    /// Deletes a chapter and updates the active document selection gracefully.
    pub fn delete_chapter(&mut self, chapter_idx: usize) {
        if matches!(
            self.selected_doc,
            Some(SelectedDocument::Scene { chapter_idx: c, .. }) if c == chapter_idx
        ) {
            self.is_dirty = false;
            self.current_file_path = None;
        } else {
            self.save_current_file_if_dirty();
        }

        let res = if let Some(p) = &mut self.project {
            let ch_title = p
                .chapters
                .get(chapter_idx)
                .map(|c| c.title.clone())
                .unwrap_or_default();
            p.delete_chapter(chapter_idx).map(|()| ch_title)
        } else {
            return;
        };

        match res {
            Ok(ch_title) => {
                self.reselect_valid_document();
                self.set_status(format!("Deleted chapter '{}'", ch_title));
            }
            Err(e) => {
                self.set_status(format!("Error deleting chapter: {}", e));
            }
        }
    }

    /// Deletes a scene and updates the active document selection gracefully.
    pub fn delete_scene(&mut self, chapter_idx: usize, scene_idx: usize) {
        if matches!(
            self.selected_doc,
            Some(SelectedDocument::Scene { chapter_idx: c, scene_idx: s }) if c == chapter_idx && s == scene_idx
        ) {
            self.is_dirty = false;
            self.current_file_path = None;
        } else {
            self.save_current_file_if_dirty();
        }

        let res = if let Some(p) = &mut self.project {
            let sc_title = p
                .chapters
                .get(chapter_idx)
                .and_then(|c| c.scenes.get(scene_idx))
                .map(|s| s.title.clone())
                .unwrap_or_default();
            p.delete_scene(chapter_idx, scene_idx).map(|()| sc_title)
        } else {
            return;
        };

        match res {
            Ok(sc_title) => {
                self.reselect_valid_document();
                self.set_status(format!("Deleted scene '{}'", sc_title));
            }
            Err(e) => {
                self.set_status(format!("Error deleting scene: {}", e));
            }
        }
    }

    /// Ensures `selected_doc` points to an existing file after a deletion.
    fn reselect_valid_document(&mut self) {
        let Some(p) = &self.project else {
            self.selected_doc = None;
            self.current_file_path = None;
            self.editor_content.clear();
            self.cached_word_count = 0;
            return;
        };

        for (ch_idx, ch) in p.chapters.iter().enumerate() {
            if !ch.scenes.is_empty() {
                self.select_scene(ch_idx, 0);
                return;
            }
        }

        if !p.front_matter.is_empty() {
            self.select_front_matter(0);
        } else {
            self.selected_doc = None;
            self.current_file_path = None;
            self.editor_content.clear();
            self.cached_word_count = 0;
            self.is_dirty = false;
        }
    }

    pub fn compile_pdf(&mut self) {
        self.save_current_file_if_dirty();

        let Some(p) = &self.project else {
            self.set_status("No project loaded to compile");
            return;
        };

        let default_name = format!("{}.pdf", sanitize_filename(&p.title));
        let dialog = rfd::FileDialog::new()
            .set_title("Compile Novel to Print-Ready 6x9 PDF")
            .add_filter("PDF Document (*.pdf)", &["pdf"])
            .set_file_name(&default_name);

        if let Some(save_path) = dialog.save_file() {
            if let Some(p) = &self.project {
                let result = compile_to_pdf(p, &save_path)
                    .map(|_| ())
                    .or_else(|_err| compile_book_to_pdf(p, &save_path));

                match result {
                    Ok(()) => {
                        self.set_status(format!(
                            "Compiled 6x9 PDF to {:?}",
                            save_path.file_name().unwrap_or_default()
                        ));
                    }
                    Err(e) => {
                        self.set_status(format!("PDF compilation failed: {}", e));
                    }
                }
            }
        }
    }

    pub fn open_project_dialog(&mut self) {
        self.save_current_file_if_dirty();
        if let Some(folder) = rfd::FileDialog::new()
            .set_title("Open Book Project Folder")
            .pick_folder()
        {
            if folder.join("book.toml").exists() {
                if let Err(e) = self.load_project(&folder) {
                    self.set_status(format!("Failed to open project: {}", e));
                }
            } else if ScrivenerImporter::resolve_scriv_paths(&folder).is_ok() {
                // If user selected a .scriv folder directly, open the Scrivener importer modal for it
                self.open_scrivener_import_modal(Some(folder));
            } else {
                self.set_status("Selected folder does not contain a book.toml file");
            }
        }
    }

    /// Returns the display title of the currently active document for the Scrivener center header box.
    pub fn active_document_title(&self) -> String {
        let Some(p) = &self.project else {
            return "No Project".to_string();
        };
        match &self.selected_doc {
            Some(SelectedDocument::FrontMatter(idx)) => p
                .front_matter
                .get(*idx)
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "Front Matter".to_string()),
            Some(SelectedDocument::Scene {
                chapter_idx,
                scene_idx,
            }) => p
                .chapters
                .get(*chapter_idx)
                .and_then(|c| c.scenes.get(*scene_idx))
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "Scene".to_string()),
            None => p.title.clone(),
        }
    }

    /// Wraps or unwraps the currently selected text in the central editor with markdown formatting (e.g. `**` or `*`).
    /// If no text is selected, inserts a formatted placeholder at cursor position.
    /// Reorders chapters via drag-and-drop and persists to `book.toml`.
    pub fn reorder_chapter_ui(&mut self, from_idx: usize, to_idx: usize) {
        if from_idx == to_idx {
            return;
        }
        if self.is_dirty {
            self.save_current_file();
        }
        if let Some(p) = &mut self.project {
            if let Err(e) = p.reorder_chapter(from_idx, to_idx) {
                self.set_status(format!("Error reordering chapters: {}", e));
                return;
            }
            if let Some(SelectedDocument::Scene { chapter_idx, scene_idx }) = self.selected_doc {
                let new_ch = if chapter_idx == from_idx {
                    to_idx
                } else if from_idx < to_idx && chapter_idx > from_idx && chapter_idx <= to_idx {
                    chapter_idx - 1
                } else if from_idx > to_idx && chapter_idx >= to_idx && chapter_idx < from_idx {
                    chapter_idx + 1
                } else {
                    chapter_idx
                };
                self.selected_doc = Some(SelectedDocument::Scene { chapter_idx: new_ch, scene_idx });
            }
            self.set_status("Chapters reordered");
        }
    }

    /// Moves a scene (within the same chapter or across chapters) via drag-and-drop and persists to disk and `book.toml`.
    pub fn reorder_scene_ui(&mut self, from_ch: usize, from_sc: usize, to_ch: usize, to_sc: usize) {
        if from_ch == to_ch && from_sc == to_sc {
            return;
        }
        if self.is_dirty {
            self.save_current_file();
        }
        if let Some(p) = &mut self.project {
            match p.reorder_scene(from_ch, from_sc, to_ch, to_sc) {
                Ok(new_path) => {
                    if let Some(SelectedDocument::Scene { chapter_idx, scene_idx }) = self.selected_doc {
                        if chapter_idx == from_ch && scene_idx == from_sc {
                            let dest_len = p.chapters[to_ch].scenes.len();
                            self.selected_doc = Some(SelectedDocument::Scene {
                                chapter_idx: to_ch,
                                scene_idx: to_sc.min(dest_len.saturating_sub(1)),
                            });
                            self.current_file_path = Some(new_path);
                        } else if chapter_idx == from_ch && from_ch == to_ch {
                            let new_sc = if from_sc < to_sc && scene_idx > from_sc && scene_idx <= to_sc {
                                scene_idx - 1
                            } else if from_sc > to_sc && scene_idx >= to_sc && scene_idx < from_sc {
                                scene_idx + 1
                            } else {
                                scene_idx
                            };
                            self.selected_doc = Some(SelectedDocument::Scene {
                                chapter_idx,
                                scene_idx: new_sc,
                            });
                        } else if chapter_idx == from_ch {
                            if scene_idx > from_sc {
                                self.selected_doc = Some(SelectedDocument::Scene {
                                    chapter_idx,
                                    scene_idx: scene_idx - 1,
                                });
                            }
                        } else if chapter_idx == to_ch {
                            if scene_idx >= to_sc {
                                self.selected_doc = Some(SelectedDocument::Scene {
                                    chapter_idx,
                                    scene_idx: scene_idx + 1,
                                });
                            }
                        }
                    }
                    self.set_status("Scenes reordered");
                }
                Err(e) => {
                    self.set_status(format!("Error moving scene: {}", e));
                }
            }
        }
    }

    pub fn apply_format_wrapper(&mut self, ctx: &egui::Context, prefix: &str, suffix: &str) {
        let editor_id = egui::Id::new("central_editor_text");
        let mut state = egui::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();

        if let Some(range) = state.cursor.char_range() {
            let min_char = range.primary.index.min(range.secondary.index);
            let max_char = range.primary.index.max(range.secondary.index);

            let start_byte = char_idx_to_byte_idx(&self.editor_content, min_char);
            let end_byte = char_idx_to_byte_idx(&self.editor_content, max_char);

            if min_char < max_char && start_byte < end_byte && end_byte <= self.editor_content.len()
            {
                let selected = &self.editor_content[start_byte..end_byte];
                if selected.starts_with(prefix)
                    && selected.ends_with(suffix)
                    && selected.len() >= prefix.len() + suffix.len()
                {
                    let unwrapped_str =
                        selected[prefix.len()..selected.len() - suffix.len()].to_string();
                    self.editor_content
                        .replace_range(start_byte..end_byte, &unwrapped_str);
                    let new_char_count = unwrapped_str.chars().count();
                    let new_primary = egui::text::CCursor::new(min_char + new_char_count);
                    let new_secondary = egui::text::CCursor::new(min_char);
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            new_secondary,
                            new_primary,
                        )));
                } else {
                    let wrapped = format!("{}{}{}", prefix, selected, suffix);
                    self.editor_content
                        .replace_range(start_byte..end_byte, &wrapped);
                    let wrapped_char_count = wrapped.chars().count();
                    let new_primary = egui::text::CCursor::new(min_char + wrapped_char_count);
                    let new_secondary = egui::text::CCursor::new(min_char);
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            new_secondary,
                            new_primary,
                        )));
                }
            } else {
                let placeholder = if prefix == "**" {
                    "bold text"
                } else if prefix == "<u>" {
                    "underlined text"
                } else {
                    "italic text"
                };
                let insertion = format!("{}{}{}", prefix, placeholder, suffix);
                let insert_pos = start_byte.min(self.editor_content.len());
                self.editor_content.insert_str(insert_pos, &insertion);

                let prefix_chars = prefix.chars().count();
                let inner_chars = placeholder.chars().count();
                let new_secondary = egui::text::CCursor::new(min_char + prefix_chars);
                let new_primary =
                    egui::text::CCursor::new(min_char + prefix_chars + inner_chars);
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        new_secondary,
                        new_primary,
                    )));
            }
        } else {
            let placeholder = if prefix == "**" {
                "bold text"
            } else if prefix == "<u>" {
                "underlined text"
            } else {
                "italic text"
            };
            self.editor_content
                .push_str(&format!("\n{}{}{}\n", prefix, placeholder, suffix));
        }

        state.store(ctx, editor_id);
        self.recompute_word_count();
        self.is_dirty = true;
        self.last_edit_time = Some(Instant::now());
    }
}

impl Drop for BookApp {
    fn drop(&mut self) {
        self.save_current_file_if_dirty();
    }
}

impl eframe::App for BookApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Match Scrivener light background so swapchain buffer transitions never flash black
        SCRIV_CANVAS_BG.to_normalized_gamma_f32()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1. Keyboard shortcuts: Ctrl+B (Bold), Ctrl+I (Italic), Ctrl+U (Underline), Ctrl+S (Save)
        let (trigger_bold, trigger_italic, trigger_underline, trigger_save) = ctx.input_mut(|i| {
            let b = i.consume_key(egui::Modifiers::COMMAND, egui::Key::B)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::B);
            let it = i.consume_key(egui::Modifiers::COMMAND, egui::Key::I)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::I);
            let u = i.consume_key(egui::Modifiers::COMMAND, egui::Key::U)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::U);
            let s = i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)
                || i.consume_key(egui::Modifiers::CTRL, egui::Key::S);
            (b, it, u, s)
        });

        if trigger_bold {
            self.is_bold = !self.is_bold;
            self.apply_format_wrapper(ctx, "**", "**");
        }

        if trigger_italic {
            self.is_italic = !self.is_italic;
            self.apply_format_wrapper(ctx, "*", "*");
        }

        if trigger_underline {
            self.is_underline = !self.is_underline;
            self.apply_format_wrapper(ctx, "<u>", "</u>");
        }

        if trigger_save {
            self.save_current_file();
        }

        // 2. Auto-save on window close request
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_current_file_if_dirty();
        }

        // 3. Periodic auto-save 2 seconds after user stops typing
        if self.is_dirty {
            if let Some(last_edit) = self.last_edit_time {
                if last_edit.elapsed() >= std::time::Duration::from_secs(2) {
                    self.save_current_file();
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_millis(500));
                }
            }
        }

        // 4. Render modal dialogs
        self.render_modals(ctx);

        // 5. Scrivener 3 Top Menu Bar (File, Edit, Insert, View, Project, Format, Compile)
        egui::TopBottomPanel::top("scrivener_menu_bar")
            .exact_height(24.0)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(248, 250, 248))
                    .inner_margin(egui::Margin::symmetric(8, 2))
                    .stroke(egui::Stroke::new(1.0_f32, SCRIV_BORDER)),
            )
            .show(ctx, |ui| {
                self.render_menu_bar(ui);
            });

        // 6. Scrivener 3 Main Application Toolbar (Binder toggle, + Add, Trash, Center Active Title Box, View Modes, Compile)
        egui::TopBottomPanel::top("scrivener_main_toolbar")
            .exact_height(38.0)
            .frame(
                egui::Frame::NONE
                    .fill(SCRIV_TOOLBAR_BG)
                    .inner_margin(egui::Margin::symmetric(10, 4))
                    .stroke(egui::Stroke::new(1.0_f32, SCRIV_BORDER)),
            )
            .show(ctx, |ui| {
                self.render_scrivener_main_toolbar(ui);
            });

        // 7. Collapsible Left Binder Panel (Scrivener 3 Binder with bottom action bar)
        if !self.sidebar_collapsed {
            egui::SidePanel::left("scrivener_binder_panel")
                .resizable(true)
                .default_width(self.sidebar_width)
                .min_width(195.0)
                .max_width(420.0)
                .frame(
                    egui::Frame::NONE
                        .fill(SCRIV_BINDER_BG)
                        .stroke(egui::Stroke::new(1.0_f32, SCRIV_BORDER)),
                )
                .show(ctx, |ui| {
                    self.render_sidebar(ui);
                });
        }

        // 8. Scrivener 3 Rich Text Formatting Toolbar
        // Hoisted to root panel level on ctx to guarantee the writing canvas cannot bleed over the topside
        egui::TopBottomPanel::top("scrivener_format_bar")
            .exact_height(30.0)
            .frame(
                egui::Frame::NONE
                    .fill(SCRIV_FORMAT_BAR_BG)
                    .inner_margin(egui::Margin::symmetric(10, 3))
                    .stroke(egui::Stroke::new(0.5_f32, SCRIV_BORDER)),
            )
            .show(ctx, |ui| {
                self.render_format_bar(ui);
            });

        // 9. Scrivener Document Header Bar (`< > 📄 Scene 1 ✏` + writer rename & print badge)
        // Hoisted to root panel level on ctx to guarantee the writing canvas cannot bleed over the topside
        egui::TopBottomPanel::top("scrivener_doc_header_bar")
            .exact_height(27.0)
            .frame(
                egui::Frame::NONE
                    .fill(SCRIV_HEADER_BAR_BG)
                    .inner_margin(egui::Margin::symmetric(10, 3))
                    .stroke(egui::Stroke::new(0.5_f32, SCRIV_BORDER)),
            )
            .show(ctx, |ui| {
                self.render_doc_header_bar(ui);
            });

        // 10. Scrivener Editor Bottom Footer Bar (Zoom, Word Count, Save status)
        // Docked at the bottom before CentralPanel to guarantee no bottom bleeding
        egui::TopBottomPanel::bottom("scrivener_editor_footer")
            .exact_height(26.0)
            .frame(
                egui::Frame::NONE
                    .fill(SCRIV_TOOLBAR_BG)
                    .inner_margin(egui::Margin::symmetric(10, 2))
                    .stroke(egui::Stroke::new(1.0_f32, SCRIV_BORDER)),
            )
            .show(ctx, |ui| {
                self.render_editor_footer(ui);
            });

        // 11. Central Writing Canvas
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(SCRIV_CANVAS_BG))
            .show(ctx, |ui| {
                self.render_central_pane(ui);
            });
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_current_file_if_dirty();
    }
}

impl BookApp {
    /// Renders the Scrivener 3 classic top menu bar (`File  Edit  Insert  View  Navigate  Project  Documents  Format  Help`).
    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.style_mut().visuals.button_frame = false;

            ui.menu_button("File", |ui| {
                if ui.button("✨ New Novel Blueprint...").clicked() {
                    self.show_new_blueprint_modal = true;
                    ui.close_menu();
                }
                if ui.button("📂 Open Project Folder...").clicked() {
                    ui.close_menu();
                    self.open_project_dialog();
                }
                if ui.button("📥 Import Scrivener Project (.scriv)...").clicked() {
                    ui.close_menu();
                    self.open_scrivener_import_modal(None);
                }
                ui.separator();
                if ui.button("💾 Save (Ctrl+S)").clicked() {
                    self.save_current_file();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("📖 Project Settings (Title & Author)...").clicked() {
                    self.open_project_metadata_modal();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("📄 Compile Print-Ready 6x9 PDF...").clicked() {
                    ui.close_menu();
                    self.compile_pdf();
                }
            });

            ui.menu_button("Edit", |ui| {
                if let Some(doc) = &self.selected_doc {
                    match *doc {
                        SelectedDocument::Scene {
                            chapter_idx,
                            scene_idx,
                        } => {
                            if ui.button("✏ Rename Active Scene (Writer View)...").clicked() {
                                if let Some(p) = &self.project {
                                    if let Some(sc) = p
                                        .chapters
                                        .get(chapter_idx)
                                        .and_then(|c| c.scenes.get(scene_idx))
                                    {
                                        self.rename_target = Some(RenameTarget::Scene {
                                            chapter_idx,
                                            scene_idx,
                                        });
                                        self.rename_input = sc.title.clone();
                                    }
                                }
                                ui.close_menu();
                            }
                            if ui
                                .button("✏ Rename Active Chapter (Writer View)...")
                                .clicked()
                            {
                                if let Some(p) = &self.project {
                                    if let Some(ch) = p.chapters.get(chapter_idx) {
                                        self.rename_target =
                                            Some(RenameTarget::Chapter(chapter_idx));
                                        self.rename_input = ch.title.clone();
                                    }
                                }
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("🗑 Delete Active Scene...").clicked() {
                                self.delete_target = Some(DeleteTarget::Scene {
                                    chapter_idx,
                                    scene_idx,
                                });
                                ui.close_menu();
                            }
                            if ui.button("🗑 Delete Active Chapter...").clicked() {
                                self.delete_target = Some(DeleteTarget::Chapter(chapter_idx));
                                ui.close_menu();
                            }
                        }
                        SelectedDocument::FrontMatter(_) => {
                            ui.label(egui::RichText::new("Front Matter selected").weak());
                        }
                    }
                }
            });

            ui.menu_button("Insert", |ui| {
                if ui.button("➕ New Scene").clicked() {
                    self.add_scene();
                    ui.close_menu();
                }
                if ui.button("📁 New Numbered Chapter").clicked() {
                    self.add_chapter();
                    ui.close_menu();
                }
                if ui
                    .button("★ New Specialized Chapter (Prologue, Epilogue...)")
                    .clicked()
                {
                    self.show_specialized_modal = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                let label = if self.sidebar_collapsed {
                    "Show Binder"
                } else {
                    "Hide Binder"
                };
                if ui.button(label).clicked() {
                    self.sidebar_collapsed = !self.sidebar_collapsed;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Zoom In (125%)").clicked() {
                    self.zoom_percent = (self.zoom_percent + 25).min(200);
                    ui.close_menu();
                }
                if ui.button("Zoom Reset (100%)").clicked() {
                    self.zoom_percent = 100;
                    ui.close_menu();
                }
                if ui.button("Zoom Out (75%)").clicked() {
                    self.zoom_percent = self.zoom_percent.saturating_sub(25).max(75);
                    ui.close_menu();
                }
            });

            ui.menu_button("Navigate", |ui| {
                if ui
                    .add_enabled(self.nav_history_pos > 0, egui::Button::new("◀ Back"))
                    .clicked()
                {
                    self.navigate_back();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(
                        self.nav_history_pos + 1 < self.nav_history.len(),
                        egui::Button::new("▶ Forward"),
                    )
                    .clicked()
                {
                    self.navigate_forward();
                    ui.close_menu();
                }
            });

            ui.menu_button("Project", |ui| {
                if ui.button("📖 Project Settings (Title & Author)...").clicked() {
                    self.open_project_metadata_modal();
                    ui.close_menu();
                }
                if ui.button("📥 Import Scrivener Project (.scriv)...").clicked() {
                    ui.close_menu();
                    self.open_scrivener_import_modal(None);
                }
                ui.separator();
                if ui.button("➕ Add Chapter").clicked() {
                    self.add_chapter();
                    ui.close_menu();
                }
                if ui.button("★ Add Specialized Chapter...").clicked() {
                    self.show_specialized_modal = true;
                    ui.close_menu();
                }
                if ui.button("📄 Compile Manuscript to 6x9 PDF...").clicked() {
                    ui.close_menu();
                    self.compile_pdf();
                }
            });

            ui.menu_button("Format", |ui| {
                if ui.button("Bold (Ctrl+B)").clicked() {
                    self.is_bold = !self.is_bold;
                    self.apply_format_wrapper(ui.ctx(), "**", "**");
                    ui.close_menu();
                }
                if ui.button("Italic (Ctrl+I)").clicked() {
                    self.is_italic = !self.is_italic;
                    self.apply_format_wrapper(ui.ctx(), "*", "*");
                    ui.close_menu();
                }
                if ui.button("Underline (Ctrl+U)").clicked() {
                    self.is_underline = !self.is_underline;
                    self.apply_format_wrapper(ui.ctx(), "<u>", "</u>");
                    ui.close_menu();
                }
                ui.separator();
                ui.checkbox(&mut self.markdown_in_editor, "Markdown In Editor");
                ui.separator();
                ui.label(egui::RichText::new("Font Family").weak().size(11.0));
                if ui
                    .radio_value(&mut self.font_family, FontFamilyChoice::Serif, "Sitka Text (Serif)")
                    .clicked()
                {
                    ui.close_menu();
                }
                if ui
                    .radio_value(
                        &mut self.font_family,
                        FontFamilyChoice::Proportional,
                        "Segoe UI (Sans-Serif)",
                    )
                    .clicked()
                {
                    ui.close_menu();
                }
                if ui
                    .radio_value(&mut self.font_family, FontFamilyChoice::Monospace, "Consolas (Monospace)")
                    .clicked()
                {
                    ui.close_menu();
                }
            });

            // Right side of menu bar: subtle project title & save status
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some((msg, time)) = &self.status_message {
                    if time.elapsed().as_secs() < 5 {
                        ui.label(
                            egui::RichText::new(format!("• {msg}"))
                                .color(SCRIV_FOLDER_BLUE)
                                .size(11.0),
                        );
                        ui.separator();
                    }
                }
                if let Some(p) = &self.project {
                    if ui
                        .small_button(
                            egui::RichText::new(format!("{} — {}", p.title, p.author))
                                .color(SCRIV_TEXT_MUTED)
                                .size(11.0),
                        )
                        .on_hover_text("Click to edit Book Title & Author...")
                        .clicked()
                    {
                        self.open_project_metadata_modal();
                    }
                }
            });
        });
    }

    /// Renders The Scribe's Ledger main toolbar with sidebar toggle on left, centered active document title, and compile/save on right.
    fn render_scrivener_main_toolbar(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width();

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Left: Sidebar Toggle Button
            let binder_btn = egui::Button::new(
                egui::RichText::new("◧")
                    .size(15.0)
                    .color(if !self.sidebar_collapsed {
                        SCRIV_FOLDER_BLUE
                    } else {
                        SCRIV_TEXT_DARK
                    }),
            )
            .min_size(egui::vec2(28.0, 26.0));
            if ui
                .add(binder_btn)
                .on_hover_text("Toggle Binder Sidebar")
                .clicked()
            {
                self.sidebar_collapsed = !self.sidebar_collapsed;
            }

            // Center Scrivener Title Box
            let center_box_w = (avail_w * 0.32).clamp(180.0, 380.0);
            let current_x = ui.cursor().min.x;
            let target_x = (avail_w * 0.5 - center_box_w * 0.5).max(current_x + 8.0);
            ui.add_space(target_x - current_x);

            egui::Frame::NONE
                .fill(egui::Color32::WHITE)
                .stroke(egui::Stroke::new(1.0_f32, SCRIV_BORDER))
                .corner_radius(2.0)
                .inner_margin(egui::Margin::symmetric(12, 4))
                .show(ui, |ui| {
                    ui.set_width(center_box_w);
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            let active_title = self.active_document_title();
                            ui.label(
                                egui::RichText::new(active_title)
                                    .size(12.5)
                                    .color(SCRIV_TEXT_DARK),
                            );
                        },
                    );
                });

            // Right cluster: Compile PDF button + Save
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                let compile_btn = egui::Button::new(
                    egui::RichText::new("🖨 Compile PDF")
                        .size(12.0)
                        .strong()
                        .color(egui::Color32::WHITE),
                )
                .fill(SCRIV_FOLDER_BLUE)
                .min_size(egui::vec2(104.0, 26.0));

                if ui
                    .add(compile_btn)
                    .on_hover_text("Compile Print-Ready 6x9 PDF Manuscript")
                    .clicked()
                {
                    self.compile_pdf();
                }

                if self.is_dirty {
                    if ui
                        .button(
                            egui::RichText::new("💾 Save")
                                .size(11.5)
                                .color(LEDGER_GREEN_PRIMARY),
                        )
                        .on_hover_text("Save changes now (Ctrl+S)")
                        .clicked()
                    {
                        self.save_current_file();
                    }
                }
            });
        });
    }

    /// Renders The Scribe's Ledger left Binder panel with drag-and-drop reordering for chapters and scenes.
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        let mut action: Option<SidebarAction> = None;

        // Top Binder Header ("The Scribe's Ledger" banner)
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(232, 238, 234))
            .inner_margin(egui::Margin::symmetric(10, 6))
            .stroke(egui::Stroke::new(0.5_f32, SCRIV_BORDER))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("📐")
                            .color(SCRIV_FOLDER_BLUE)
                            .size(13.0),
                    );
                    let title = self
                        .project
                        .as_ref()
                        .map(|p| p.title.as_str())
                        .unwrap_or("The Scribe's Ledger");
                    ui.label(
                        egui::RichText::new(title)
                            .strong()
                            .size(12.0)
                            .color(SCRIV_TEXT_DARK),
                    );
                    if self.project.is_some() {
                        if ui
                            .small_button(egui::RichText::new("✏").size(10.5))
                            .on_hover_text("Edit Book Title & Author...")
                            .clicked()
                        {
                            self.open_project_metadata_modal();
                        }
                    }
                });
            });

        // Main Binder Tree Scroll Area
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.spacing_mut().item_spacing.y = 2.0;

                // 1. MANUSCRIPT SECTION (At the top of the Binder)
                egui::CollapsingHeader::new(
                    egui::RichText::new("📂 Manuscript")
                        .strong()
                        .size(12.5)
                        .color(SCRIV_TEXT_DARK),
                )
                .default_open(true)
                .id_salt("binder_manuscript_root")
                .show(ui, |ui| {
                    if let Some(p) = &self.project {
                        let mut numbered_counter: usize = 0;

                        for (ch_idx, chapter) in p.chapters.iter().enumerate() {
                            let is_ch_active = matches!(
                                self.selected_doc,
                                Some(SelectedDocument::Scene { chapter_idx, .. }) if chapter_idx == ch_idx
                            );

                            let (folder_icon, _folder_color, print_hint) = match &chapter.kind {
                                ChapterKind::Numbered => {
                                    numbered_counter += 1;
                                    (
                                        "📁",
                                        SCRIV_FOLDER_BLUE,
                                        format!("Print Chapter #{}", numbered_counter),
                                    )
                                }
                                ChapterKind::Specialized(st) => (
                                    "★",
                                    SCRIV_SPECIAL_GOLD,
                                    format!("Specialized Print Heading: {}", st),
                                ),
                            };

                            let ch_header_text = egui::RichText::new(format!(
                                "{} {}",
                                folder_icon, chapter.title
                            ))
                            .size(12.0)
                            .color(if is_ch_active {
                                egui::Color32::from_rgb(20, 75, 42)
                            } else {
                                SCRIV_TEXT_DARK
                            });

                            let id = ui.make_persistent_id(format!("ch_collapse_{}", chapter.name));
                            let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);

                            let header_res = state.show_header(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 3.0;

                                    let grip = ui.add(
                                        egui::Label::new(egui::RichText::new("⠿").weak().size(11.0))
                                            .sense(egui::Sense::drag())
                                    )
                                    .on_hover_text("Drag to reorder chapter");

                                    if grip.dragged() {
                                        grip.dnd_set_drag_payload(BinderDragPayload::Chapter(ch_idx));
                                    }

                                    let ch_btn = egui::Button::new(
                                        if is_ch_active {
                                            ch_header_text.clone().strong()
                                        } else {
                                            ch_header_text.clone()
                                        }
                                    )
                                    .frame(false)
                                    .sense(egui::Sense::click_and_drag());

                                    let btn_resp = ui.add(ch_btn);
                                    if btn_resp.dragged() {
                                        btn_resp.dnd_set_drag_payload(BinderDragPayload::Chapter(ch_idx));
                                    }

                                    btn_resp.on_hover_text(format!(
                                        "{} ({})\nDrag to reorder chapter • Right-click for options",
                                        chapter.title, print_hint
                                    ))
                                    .context_menu(|ui| {
                                        ui.label(
                                            egui::RichText::new(&print_hint)
                                                .weak()
                                                .italics()
                                                .size(11.0),
                                        );
                                        ui.separator();
                                        if ui.button("✏ Rename Chapter (Writer View)").clicked() {
                                            action = Some(SidebarAction::StartRenameChapter(
                                                ch_idx,
                                                chapter.title.clone(),
                                            ));
                                            ui.close_menu();
                                        }
                                        if ui.button("➕ Add Scene to Chapter").clicked() {
                                            action = Some(SidebarAction::SelectScene(ch_idx, 0));
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui
                                            .button(
                                                egui::RichText::new("🗑 Delete Chapter")
                                                    .color(egui::Color32::from_rgb(190, 45, 45)),
                                            )
                                            .clicked()
                                        {
                                            action = Some(SidebarAction::ConfirmDeleteChapter(ch_idx));
                                            ui.close_menu();
                                        }
                                    });

                                    if is_ch_active {
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui
                                                .small_button(egui::RichText::new("×").size(11.0))
                                                .on_hover_text("Delete chapter")
                                                .clicked()
                                            {
                                                action = Some(SidebarAction::ConfirmDeleteChapter(ch_idx));
                                            }
                                            if ui
                                                .small_button(egui::RichText::new("✏").size(10.0))
                                                .on_hover_text("Rename chapter (writer's view only)")
                                                .clicked()
                                            {
                                                action = Some(SidebarAction::StartRenameChapter(
                                                    ch_idx,
                                                    chapter.title.clone(),
                                                ));
                                            }
                                        });
                                    }
                                });
                            });

                            let (_toggle_resp, header_inner, _body_resp) = header_res.body(|ui| {
                                for (sc_idx, scene) in chapter.scenes.iter().enumerate() {
                                    let is_selected = matches!(
                                        self.selected_doc,
                                        Some(SelectedDocument::Scene {
                                            chapter_idx,
                                            scene_idx,
                                        }) if chapter_idx == ch_idx && scene_idx == sc_idx
                                    );

                                    self.render_binder_scene_row(
                                        ui,
                                        ch_idx,
                                        sc_idx,
                                        &scene.title,
                                        is_selected,
                                        &mut action,
                                    );
                                }

                                let (end_sc_rect, end_sc_resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
                                if let Some(payload) = end_sc_resp.dnd_hover_payload::<BinderDragPayload>() {
                                    if matches!(*payload, BinderDragPayload::Scene { .. }) {
                                        ui.painter().hline(
                                            end_sc_rect.x_range(),
                                            end_sc_rect.center().y,
                                            egui::Stroke::new(2.0_f32, LEDGER_GREEN_PRIMARY),
                                        );
                                    }
                                }
                                if let Some(payload) = end_sc_resp.dnd_release_payload::<BinderDragPayload>() {
                                    if let BinderDragPayload::Scene { chapter_idx: from_ch, scene_idx: from_sc } = *payload {
                                        action = Some(SidebarAction::MoveScene {
                                            from_ch,
                                            from_sc,
                                            to_ch: ch_idx,
                                            to_sc: chapter.scenes.len(),
                                        });
                                    }
                                }
                            });

                            let header_resp = header_inner.response;

                            if let Some(_payload) = header_resp.dnd_hover_payload::<BinderDragPayload>() {
                                ui.painter().hline(
                                    header_resp.rect.x_range(),
                                    header_resp.rect.top(),
                                    egui::Stroke::new(2.5_f32, LEDGER_GREEN_PRIMARY),
                                );
                            }

                            if let Some(payload) = header_resp.dnd_release_payload::<BinderDragPayload>() {
                                match *payload {
                                    BinderDragPayload::Chapter(from_ch) => {
                                        if from_ch != ch_idx {
                                            action = Some(SidebarAction::MoveChapter { from: from_ch, to: ch_idx });
                                        }
                                    }
                                    BinderDragPayload::Scene { chapter_idx: from_ch, scene_idx: from_sc } => {
                                        action = Some(SidebarAction::MoveScene {
                                            from_ch,
                                            from_sc,
                                            to_ch: ch_idx,
                                            to_sc: chapter.scenes.len(),
                                        });
                                    }
                                }
                            }
                        }

                        let (end_rect, end_resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 12.0), egui::Sense::hover());
                        if let Some(payload) = end_resp.dnd_hover_payload::<BinderDragPayload>() {
                            if matches!(*payload, BinderDragPayload::Chapter(_)) {
                                ui.painter().hline(
                                    end_rect.x_range(),
                                    end_rect.center().y,
                                    egui::Stroke::new(2.5_f32, LEDGER_GREEN_PRIMARY),
                                );
                            }
                        }
                        if let Some(payload) = end_resp.dnd_release_payload::<BinderDragPayload>() {
                            if let BinderDragPayload::Chapter(from_ch) = *payload {
                                let last_idx = p.chapters.len().saturating_sub(1);
                                if from_ch != last_idx {
                                    action = Some(SidebarAction::MoveChapter { from: from_ch, to: last_idx });
                                }
                            }
                        }
                    }
                });

                ui.add_space(6.0);

                // 2. FRONT MATTER SECTION (Below Manuscript)
                egui::CollapsingHeader::new(
                    egui::RichText::new("📑 Front Matter")
                        .strong()
                        .size(12.5)
                        .color(SCRIV_TEXT_DARK),
                )
                .default_open(true)
                .id_salt("binder_front_matter_root")
                .show(ui, |ui| {
                    egui::CollapsingHeader::new(
                        egui::RichText::new("📁 Paperback (6\" × 9\")")
                            .size(12.0)
                            .color(SCRIV_TEXT_DARK),
                    )
                    .default_open(true)
                    .id_salt("binder_paperback_sub")
                    .show(ui, |ui| {
                        if let Some(p) = &self.project {
                            for (idx, item) in p.front_matter.iter().enumerate() {
                                let is_selected = matches!(
                                    self.selected_doc,
                                    Some(SelectedDocument::FrontMatter(i)) if i == idx
                                );

                                let row_fill = if is_selected {
                                    SCRIV_BINDER_SELECTED_BG
                                } else {
                                    egui::Color32::TRANSPARENT
                                };

                                egui::Frame::NONE
                                    .fill(row_fill)
                                    .corner_radius(2.0)
                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                    .show(ui, |ui| {
                                        let label = egui::Button::new(
                                            egui::RichText::new(format!("📄 {}", item.title))
                                                .size(12.0)
                                                .color(SCRIV_TEXT_DARK),
                                        )
                                        .frame(false);
                                        if ui.add(label).clicked() {
                                            action = Some(SidebarAction::SelectFrontMatter(idx));
                                        }
                                    });
                            }
                        }
                    });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(4.0);

                // Quick Accessible Buttons for + Chapter, + Special, + Scene
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if ui
                        .small_button("+ Chapter")
                        .on_hover_text("Add a new numbered chapter")
                        .clicked()
                    {
                        self.add_chapter();
                    }
                    if ui
                        .small_button("+ Special")
                        .on_hover_text("Add Prologue, Epilogue, Writer's Note, etc.")
                        .clicked()
                    {
                        self.show_specialized_modal = true;
                    }
                    if ui
                        .small_button("+ Scene")
                        .on_hover_text("Add a new scene to the active chapter")
                        .clicked()
                    {
                        self.add_scene();
                    }
                });
            });

        // Execute deferred sidebar action outside immutable borrow of project
        if let Some(act) = action {
            match act {
                SidebarAction::SelectFrontMatter(idx) => self.select_front_matter(idx),
                SidebarAction::SelectScene(ch, sc) => self.select_scene(ch, sc),
                SidebarAction::StartRenameChapter(ch_idx, current_title) => {
                    self.rename_target = Some(RenameTarget::Chapter(ch_idx));
                    self.rename_input = current_title;
                }
                SidebarAction::StartRenameScene(ch_idx, sc_idx, current_title) => {
                    self.rename_target = Some(RenameTarget::Scene {
                        chapter_idx: ch_idx,
                        scene_idx: sc_idx,
                    });
                    self.rename_input = current_title;
                }
                SidebarAction::ConfirmDeleteChapter(ch_idx) => {
                    self.delete_target = Some(DeleteTarget::Chapter(ch_idx));
                }
                SidebarAction::ConfirmDeleteScene(ch_idx, sc_idx) => {
                    self.delete_target = Some(DeleteTarget::Scene {
                        chapter_idx: ch_idx,
                        scene_idx: sc_idx,
                    });
                }
                SidebarAction::MoveChapter { from, to } => {
                    self.reorder_chapter_ui(from, to);
                }
                SidebarAction::MoveScene { from_ch, from_sc, to_ch, to_sc } => {
                    self.reorder_scene_ui(from_ch, from_sc, to_ch, to_sc);
                }
            }
        }
    }

    /// Renders a single scene item row in the Binder with drag-and-drop reordering, selection highlight, and context actions.
    fn render_binder_scene_row(
        &self,
        ui: &mut egui::Ui,
        ch_idx: usize,
        sc_idx: usize,
        scene_title: &str,
        is_selected: bool,
        action: &mut Option<SidebarAction>,
    ) {
        let row_fill = if is_selected {
            SCRIV_BINDER_SELECTED_BG
        } else {
            egui::Color32::TRANSPARENT
        };

        let row_resp = egui::Frame::NONE
            .fill(row_fill)
            .corner_radius(2.0)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    // Drag grip handle
                    let handle = ui.add(
                        egui::Label::new(egui::RichText::new("⠿").weak().size(11.0))
                            .sense(egui::Sense::drag())
                    )
                    .on_hover_text("Drag to rearrange scene");

                    if handle.dragged() {
                        handle.dnd_set_drag_payload(BinderDragPayload::Scene {
                            chapter_idx: ch_idx,
                            scene_idx: sc_idx,
                        });
                    }

                    let text = egui::RichText::new(format!("📄 {}", scene_title))
                        .size(12.0)
                        .color(SCRIV_TEXT_DARK);
                    let btn = egui::Button::new(if is_selected { text.strong() } else { text })
                        .frame(false)
                        .sense(egui::Sense::click_and_drag());

                    let resp = ui.add(btn);
                    if resp.clicked() {
                        *action = Some(SidebarAction::SelectScene(ch_idx, sc_idx));
                    }
                    if resp.dragged() {
                        resp.dnd_set_drag_payload(BinderDragPayload::Scene {
                            chapter_idx: ch_idx,
                            scene_idx: sc_idx,
                        });
                    }

                    resp.context_menu(|ui| {
                        if ui.button("✏ Rename Scene (Writer View)").clicked() {
                            *action = Some(SidebarAction::StartRenameScene(
                                ch_idx,
                                sc_idx,
                                scene_title.to_string(),
                            ));
                            ui.close_menu();
                        }
                        if ui
                            .button(
                                egui::RichText::new("🗑 Delete Scene")
                                    .color(egui::Color32::from_rgb(190, 45, 45)),
                            )
                            .clicked()
                        {
                            *action = Some(SidebarAction::ConfirmDeleteScene(ch_idx, sc_idx));
                            ui.close_menu();
                        }
                    });

                    // Subtle inline rename/delete buttons when selected
                    if is_selected {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button(egui::RichText::new("×").size(11.0))
                                .on_hover_text("Delete scene")
                                .clicked()
                            {
                                *action = Some(SidebarAction::ConfirmDeleteScene(ch_idx, sc_idx));
                            }
                            if ui
                                .small_button(egui::RichText::new("✏").size(10.0))
                                .on_hover_text("Rename scene (writer's view only)")
                                .clicked()
                            {
                                *action = Some(SidebarAction::StartRenameScene(
                                    ch_idx,
                                    sc_idx,
                                    scene_title.to_string(),
                                ));
                            }
                        });
                    }
                });
            })
            .response;

        if let Some(_payload) = row_resp.dnd_hover_payload::<BinderDragPayload>() {
            ui.painter().hline(
                row_resp.rect.x_range(),
                row_resp.rect.top(),
                egui::Stroke::new(2.5_f32, LEDGER_GREEN_PRIMARY),
            );
        }

        if let Some(payload) = row_resp.dnd_release_payload::<BinderDragPayload>() {
            match *payload {
                BinderDragPayload::Scene { chapter_idx: from_ch, scene_idx: from_sc } => {
                    *action = Some(SidebarAction::MoveScene {
                        from_ch,
                        from_sc,
                        to_ch: ch_idx,
                        to_sc: sc_idx,
                    });
                }
                BinderDragPayload::Chapter(from_ch) => {
                    *action = Some(SidebarAction::MoveChapter {
                        from: from_ch,
                        to: ch_idx,
                    });
                }
            }
        }
    }

    /// Renders The Scribe's Ledger rich text formatting toolbar.
    fn render_format_bar(&mut self, ui: &mut egui::Ui) {
        let mut do_format_bold = false;
        let mut do_format_italic = false;
        let mut do_format_underline = false;

        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Style preset dropdown ("No Style")
            egui::ComboBox::from_id_salt("scriv_style_preset")
                .selected_text("No Style")
                .width(88.0)
                .show_ui(ui, |ui| {
                    let _ = ui.selectable_label(true, "No Style");
                    if ui.selectable_label(false, "Heading 1 (# )").clicked() {
                        self.editor_content.insert_str(0, "# ");
                        self.is_dirty = true;
                    }
                    if ui.selectable_label(false, "Heading 2 (## )").clicked() {
                        self.editor_content.insert_str(0, "## ");
                        self.is_dirty = true;
                    }
                    if ui.selectable_label(false, "Heading 3 (### )").clicked() {
                        self.editor_content.insert_str(0, "### ");
                        self.is_dirty = true;
                    }
                    if ui.selectable_label(false, "Block Quote (> )").clicked() {
                        self.editor_content.insert_str(0, "> ");
                        self.is_dirty = true;
                    }
                });

            // Font Family Dropdown ("Sitka Text" / "Segoe UI" / "Consolas")
            egui::ComboBox::from_id_salt("scriv_font_family_combo")
                .selected_text(self.font_family.name())
                .width(115.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.font_family,
                        FontFamilyChoice::Serif,
                        "Sitka Text",
                    );
                    ui.selectable_value(
                        &mut self.font_family,
                        FontFamilyChoice::Proportional,
                        "Segoe UI",
                    );
                    ui.selectable_value(
                        &mut self.font_family,
                        FontFamilyChoice::Monospace,
                        "Consolas",
                    );
                });

            // Font Size Combo
            egui::ComboBox::from_id_salt("scriv_font_size")
                .selected_text(format!("{}", self.font_size as u32))
                .width(48.0)
                .show_ui(ui, |ui| {
                    for sz in [11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 18.0, 20.0, 24.0] {
                        ui.selectable_value(
                            &mut self.font_size,
                            sz,
                            format!("{}", sz as u32),
                        );
                    }
                });

            // Line spacing selector ("1.0x", "1.2x", "1.5x")
            egui::ComboBox::from_id_salt("scriv_line_spacing")
                .selected_text(format!("{:.1}x", self.line_spacing))
                .width(54.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.line_spacing, 1.0, "1.0x");
                    ui.selectable_value(&mut self.line_spacing, 1.2, "1.2x");
                    ui.selectable_value(&mut self.line_spacing, 1.5, "1.5x");
                });

            ui.separator();

            // Bold / Italic / Underline buttons (`B` `I` `U`)
            let bold_btn = ui.selectable_label(
                self.is_bold,
                egui::RichText::new("B").strong().size(13.0),
            );
            if bold_btn.clicked() {
                self.is_bold = !self.is_bold;
                do_format_bold = true;
            }
            bold_btn.on_hover_text("Bold (Ctrl+B)");

            let italic_btn = ui.selectable_label(
                self.is_italic,
                egui::RichText::new("I").italics().size(13.0),
            );
            if italic_btn.clicked() {
                self.is_italic = !self.is_italic;
                do_format_italic = true;
            }
            italic_btn.on_hover_text("Italic (Ctrl+I)");

            let underline_btn = ui.selectable_label(
                self.is_underline,
                egui::RichText::new("U").underline().size(13.0),
            );
            if underline_btn.clicked() {
                self.is_underline = !self.is_underline;
                do_format_underline = true;
            }
            underline_btn.on_hover_text("Underline (Ctrl+U)");

            ui.separator();
            ui.checkbox(&mut self.markdown_in_editor, "Markdown In Editor")
                .on_hover_text("Toggle visibility of Markdown characters (*, <u>, #) in the editor");
        });

        if do_format_bold {
            self.apply_format_wrapper(ui.ctx(), "**", "**");
        }
        if do_format_italic {
            self.apply_format_wrapper(ui.ctx(), "*", "*");
        }
        if do_format_underline {
            self.apply_format_wrapper(ui.ctx(), "<u>", "</u>");
        }
    }

    /// Renders Scrivener 3's document header bar (`< > 📄 Scene 1 ✏` + writer rename & print badge).
    fn render_doc_header_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Back / Forward navigation buttons (`<` `>`)
            if ui
                .add_enabled(
                    self.nav_history_pos > 0,
                    egui::Button::new(egui::RichText::new("‹").size(14.0)).frame(false),
                )
                .on_hover_text("Navigate Back")
                .clicked()
            {
                self.navigate_back();
            }
            if ui
                .add_enabled(
                    self.nav_history_pos + 1 < self.nav_history.len(),
                    egui::Button::new(egui::RichText::new("›").size(14.0)).frame(false),
                )
                .on_hover_text("Navigate Forward")
                .clicked()
            {
                self.navigate_forward();
            }

            ui.add_space(2.0);
            let doc_title = self.active_document_title();
            ui.label(
                egui::RichText::new(format!("📄 {}", doc_title))
                    .size(12.5)
                    .color(SCRIV_TEXT_DARK),
            );

            // Quick inline rename button for the active scene or chapter
            if let Some(SelectedDocument::Scene {
                chapter_idx,
                scene_idx,
            }) = self.selected_doc
            {
                if ui
                    .small_button(egui::RichText::new("✏").size(10.5))
                    .on_hover_text("Edit Scene Name (Writer's View Only)")
                    .clicked()
                {
                    self.rename_target = Some(RenameTarget::Scene {
                        chapter_idx,
                        scene_idx,
                    });
                    self.rename_input = doc_title;
                }
            }

            // Right side of document header bar: Print Chapter badge & Chapter Rename
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(SelectedDocument::Scene { chapter_idx, .. }) = self.selected_doc
                {
                    if let Some(p) = &self.project {
                        if let Some(ch) = p.chapters.get(chapter_idx) {
                            let print_badge = match &ch.kind {
                                ChapterKind::Specialized(st) => {
                                    format!("Print Heading: \"{}\"", st)
                                }
                                ChapterKind::Numbered => {
                                    let num = p
                                        .chapters
                                        .iter()
                                        .take(chapter_idx + 1)
                                        .filter(|c| c.kind.is_numbered())
                                        .count();
                                    format!("Print Chapter: {}", num)
                                }
                            };
                            ui.label(
                                egui::RichText::new(format!(
                                    "📁 {}  •  {}",
                                    ch.title, print_badge
                                ))
                                .size(11.0)
                                .color(SCRIV_TEXT_MUTED),
                            );
                        }
                    }
                }
            });
        });
    }

    /// Centered White Paper Sheet with book-like margins, paper shadow, and floating desktop feel.
    fn render_central_pane(&mut self, ui: &mut egui::Ui) {
        let font_family = self.font_family.to_egui_family();
        let effective_size = self.font_size * (self.zoom_percent as f32 / 100.0);
        let line_spacing = self.line_spacing;

        let show_markdown = self.markdown_in_editor;
        let mut layouter = |ui: &egui::Ui, text: &str, wrap_width: f32| {
            highlight_markdown(
                ui,
                text,
                wrap_width,
                font_family.clone(),
                effective_size,
                line_spacing,
                show_markdown,
            )
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Top margin so the paper sheet floats gracefully below the header bar without touching or bleeding over
                ui.add_space(16.0);

                let total_width = ui.available_width();
                let desired_paper_width = 720.0 * (self.zoom_percent as f32 / 100.0);
                let side_padding = ((total_width - desired_paper_width) * 0.5).max(16.0);

                ui.horizontal(|ui| {
                    ui.add_space(side_padding);

                    egui::Frame::NONE
                        .fill(SCRIV_PAPER_BG)
                        .inner_margin(egui::Margin::symmetric(48, 32))
                        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(220, 226, 222)))
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 2],
                            blur: 6,
                            spread: 1,
                            color: egui::Color32::from_black_alpha(12),
                        })
                        .show(ui, |ui| {
                            let paper_inner_width =
                                (total_width - side_padding * 2.0 - 96.0).max(260.0);
                            ui.set_min_width(paper_inner_width);
                            ui.set_max_width(paper_inner_width);
                            ui.set_min_height(ui.available_height().max(560.0));

                            let editor = egui::TextEdit::multiline(&mut self.editor_content)
                                .id(egui::Id::new("central_editor_text"))
                                .layouter(&mut layouter)
                                .desired_width(paper_inner_width)
                                .desired_rows(28)
                                .frame(false)
                                .lock_focus(true);

                            let response = ui.add(editor);
                            if response.changed() {
                                self.recompute_word_count();
                                self.is_dirty = true;
                                self.last_edit_time = Some(Instant::now());
                            }
                        });
                });

                // Bottom margin so the paper sheet does not touch the bottom status bar directly
                ui.add_space(24.0);
            });
    }

    /// Renders Scrivener 3's bottom footer status bar (`100% ▾` zoom on left, `X words` centered, save status on right).
    /// Docked at the bottom with 26.0px height to match the binder bottom action bar.
    fn render_editor_footer(&mut self, ui: &mut egui::Ui) {
        let avail_w = ui.available_width();
        ui.horizontal_centered(|ui| {
            // Left: Zoom percentage selector ("100% ▾")
            egui::ComboBox::from_id_salt("scriv_zoom_combo")
                .selected_text(format!("{}%", self.zoom_percent))
                .width(62.0)
                .show_ui(ui, |ui| {
                    for z in [75, 100, 125, 150, 175, 200] {
                        ui.selectable_value(&mut self.zoom_percent, z, format!("{}%", z));
                    }
                });

            // Center: Formatted word count ("2,458 words")
            let current_x = ui.cursor().min.x;
            let target_x = (avail_w * 0.5 - 40.0).max(current_x + 10.0);
            ui.add_space(target_x - current_x);
            ui.label(
                egui::RichText::new(format!(
                    "{} words",
                    format_number_with_commas(self.cached_word_count)
                ))
                .size(11.5)
                .color(SCRIV_TEXT_DARK),
            );

            // Right: Save status indicator
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.is_dirty {
                    ui.label(
                        egui::RichText::new("● Unsaved")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(185, 105, 0)),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("✓ Saved")
                            .size(11.0)
                            .color(SCRIV_TEXT_MUTED),
                    );
                }
            });
        });
    }

    /// Renders modal dialogs for creating a blueprint, importing Scrivener projects, adding specialized chapters, renaming, and deleting.
    fn render_modals(&mut self, ctx: &egui::Context) {
        // 1. New Blueprint Modal
        if self.show_new_blueprint_modal {
            let mut close_modal = false;
            let mut create_blueprint = false;

            egui::Window::new("Create New Novel Blueprint")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    ui.label("Title:");
                    ui.text_edit_singleline(&mut self.new_title_input);

                    ui.label("Author:");
                    ui.text_edit_singleline(&mut self.new_author_input);

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            create_blueprint = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if create_blueprint {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Choose Project Folder")
                    .pick_folder()
                {
                    match BookProject::create_novel_blueprint(
                        &folder,
                        &self.new_title_input,
                        &self.new_author_input,
                    ) {
                        Ok(proj) => {
                            self.set_project(proj);
                            close_modal = true;
                        }
                        Err(e) => {
                            self.set_status(format!("Error creating blueprint: {}", e));
                        }
                    }
                }
            }

            if close_modal {
                self.show_new_blueprint_modal = false;
            }
        }

        // 2. Add Specialized Chapter Modal
        if self.show_specialized_modal {
            let mut close_modal = false;
            let mut confirm_create = false;

            egui::Window::new("Add Specialized Chapter")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.label(
                        "Specialized chapters do not increment chapter numbers and show their unique title in the compiled PDF.",
                    );

                    ui.radio_value(
                        &mut self.use_custom_specialized_title,
                        false,
                        "Choose from presets:",
                    );
                    if !self.use_custom_specialized_title {
                        egui::ComboBox::from_id_salt("specialized_preset_combo")
                            .selected_text(&self.selected_specialized_preset)
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for preset in ChapterKind::preset_options() {
                                    if ui
                                        .selectable_value(
                                            &mut self.selected_specialized_preset,
                                            (*preset).to_string(),
                                            *preset,
                                        )
                                        .clicked()
                                    {
                                        self.insert_specialized_at_start = matches!(
                                            *preset,
                                            "Prologue" | "Foreword" | "Preface" | "Introduction"
                                        );
                                    }
                                }
                            });
                    }

                    ui.radio_value(&mut self.use_custom_specialized_title, true, "Custom title:");
                    if self.use_custom_specialized_title {
                        ui.text_edit_singleline(&mut self.custom_specialized_title);
                    }

                    ui.add_space(4.0);
                    ui.checkbox(
                        &mut self.insert_specialized_at_start,
                        "Insert at beginning of manuscript (before Chapter 1)",
                    );

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Add Specialized Chapter").clicked() {
                            confirm_create = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if confirm_create {
                let chosen_title = if self.use_custom_specialized_title {
                    let t = self.custom_specialized_title.trim();
                    if t.is_empty() {
                        "Special Section".to_string()
                    } else {
                        t.to_string()
                    }
                } else {
                    self.selected_specialized_preset.clone()
                };
                self.add_specialized_chapter(&chosen_title, self.insert_specialized_at_start);
                close_modal = true;
            }

            if close_modal {
                self.show_specialized_modal = false;
            }
        }

        // 3. Rename Chapter / Scene Modal (Writer-Only View Name)
        if let Some(target) = self.rename_target.clone() {
            let mut close_modal = false;
            let mut apply_rename = false;

            let modal_title = match &target {
                RenameTarget::Chapter(_) => "Edit Chapter Name (Writer's View)",
                RenameTarget::Scene { .. } => "Edit Scene Name (Writer's View)",
            };

            egui::Window::new(modal_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.label(
                        egui::RichText::new(
                            "Note: This name is only for your writer's view and will not affect the compiled print PDF.",
                        )
                        .weak()
                        .size(11.5),
                    );
                    let resp = ui.text_edit_singleline(&mut self.rename_input);
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply_rename = true;
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save Name").clicked() {
                            apply_rename = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if apply_rename {
                let new_name = self.rename_input.trim().to_string();
                if !new_name.is_empty() {
                    if let Some(p) = &mut self.project {
                        let res = match target {
                            RenameTarget::Chapter(ch_idx) => p.rename_chapter(ch_idx, &new_name),
                            RenameTarget::Scene {
                                chapter_idx,
                                scene_idx,
                            } => p.rename_scene(chapter_idx, scene_idx, &new_name),
                        };
                        match res {
                            Ok(()) => self.set_status(format!("Renamed to '{}'", new_name)),
                            Err(e) => self.set_status(format!("Rename error: {}", e)),
                        }
                    }
                }
                close_modal = true;
            }

            if close_modal {
                self.rename_target = None;
            }
        }

        // 4. Delete Confirmation Modal
        if let Some(target) = self.delete_target.clone() {
            let mut close_modal = false;
            let mut confirm_delete = false;

            let description = match &target {
                DeleteTarget::Chapter(ch_idx) => {
                    let name = self
                        .project
                        .as_ref()
                        .and_then(|p| p.chapters.get(*ch_idx))
                        .map(|c| c.title.as_str())
                        .unwrap_or("this chapter");
                    format!(
                        "Are you sure you want to permanently delete chapter '{}' and all its scenes?",
                        name
                    )
                }
                DeleteTarget::Scene {
                    chapter_idx,
                    scene_idx,
                } => {
                    let name = self
                        .project
                        .as_ref()
                        .and_then(|p| p.chapters.get(*chapter_idx))
                        .and_then(|c| c.scenes.get(*scene_idx))
                        .map(|s| s.title.as_str())
                        .unwrap_or("this scene");
                    format!("Are you sure you want to permanently delete scene '{}'?", name)
                }
            };

            egui::Window::new("Confirm Delete")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    ui.label(description);
                    ui.separator();
                    ui.horizontal(|ui| {
                        let del_btn = egui::Button::new(
                            egui::RichText::new("🗑 Delete")
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(190, 50, 50));

                        if ui.add(del_btn).clicked() {
                            confirm_delete = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if confirm_delete {
                match target {
                    DeleteTarget::Chapter(ch_idx) => self.delete_chapter(ch_idx),
                    DeleteTarget::Scene {
                        chapter_idx,
                        scene_idx,
                    } => self.delete_scene(chapter_idx, scene_idx),
                }
                close_modal = true;
            }

            if close_modal {
                self.delete_target = None;
            }
        }

        // 5. Project Metadata Modal (Edit Book Title & Author for entire project)
        if self.show_project_metadata_modal {
            let mut close_modal = false;
            let mut save_metadata = false;

            egui::Window::new("📖 Project Metadata: Title & Author")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.label(
                        egui::RichText::new("Configure Project Title & Author")
                            .strong()
                            .size(13.0),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Saved to book.toml. $Title and $Author placeholders used in any front matter or scene files are automatically replaced with these values during compilation.",
                        )
                        .weak()
                        .size(11.0),
                    );
                    ui.separator();

                    ui.label(egui::RichText::new("Book Title:").strong());
                    ui.text_edit_singleline(&mut self.edit_project_title_input);

                    ui.label(egui::RichText::new("Author Name:").strong());
                    ui.text_edit_singleline(&mut self.edit_project_author_input);

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save Metadata").clicked() {
                            save_metadata = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if save_metadata {
                let new_title = self.edit_project_title_input.trim().to_string();
                let new_author = self.edit_project_author_input.trim().to_string();
                if !new_title.is_empty() && !new_author.is_empty() {
                    if let Some(p) = &mut self.project {
                        if let Err(e) = p.update_project_metadata(&new_title, &new_author) {
                            self.set_status(format!("Error saving metadata: {}", e));
                        } else {
                            self.set_status(format!("Updated project: \"{}\" by {}", new_title, new_author));
                        }
                    }
                    close_modal = true;
                }
            }

            if close_modal {
                self.show_project_metadata_modal = false;
            }
        }

        // 6. Import Scrivener Project (.scriv) Modal
        if self.show_scrivener_import_modal {
            let mut close_modal = false;
            let mut browse_source_folder = false;
            let mut browse_source_file = false;
            let mut browse_dest_folder = false;
            let mut run_import = false;

            egui::Window::new("📥 Import Scrivener Project (.scriv)")
                .collapsible(false)
                .resizable(false)
                .min_width(420.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    ui.label(
                        egui::RichText::new(
                            "Port a Scrivener 3 (.scriv) project into The Scribe's Ledger's Markdown & Typst format.",
                        )
                        .weak()
                        .size(11.5),
                    );
                    ui.separator();

                    // Source .scriv selector
                    ui.label(egui::RichText::new("1. Scrivener Project (.scriv folder or .scrivx file):").strong());
                    let src_display = self
                        .scriv_source_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "No .scriv project selected".to_string());
                    ui.label(egui::RichText::new(src_display).size(11.0).color(SCRIV_TEXT_MUTED));
                    ui.horizontal(|ui| {
                        if ui.button("📂 Select .scriv Folder...").clicked() {
                            browse_source_folder = true;
                        }
                        if ui.button("📄 Select .scrivx File...").clicked() {
                            browse_source_file = true;
                        }
                    });

                    ui.add_space(4.0);

                    // Destination folder selector
                    ui.label(egui::RichText::new("2. Destination Folder for Ported Scribe's Ledger Project:").strong());
                    let dst_display = self
                        .scriv_dest_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "No destination folder selected".to_string());
                    ui.label(egui::RichText::new(dst_display).size(11.0).color(SCRIV_TEXT_MUTED));
                    if ui.button("📂 Choose Destination Folder...").clicked() {
                        browse_dest_folder = true;
                    }

                    ui.add_space(4.0);
                    ui.separator();

                    // Optional Title & Author override
                    ui.label(egui::RichText::new("Book Title (optional override):").strong());
                    ui.text_edit_singleline(&mut self.scriv_title_input);

                    ui.label(egui::RichText::new("Author Name (optional override):").strong());
                    ui.text_edit_singleline(&mut self.scriv_author_input);

                    ui.separator();
                    ui.horizontal(|ui| {
                        let can_import =
                            self.scriv_source_path.is_some() && self.scriv_dest_path.is_some();
                        let import_btn = egui::Button::new(
                            egui::RichText::new("📥 Import & Open Project")
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(SCRIV_FOLDER_BLUE);

                        if ui.add_enabled(can_import, import_btn).clicked() {
                            run_import = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_modal = true;
                        }
                    });
                });

            if browse_source_folder {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Select Scrivener (.scriv) Project Folder")
                    .pick_folder()
                {
                    self.apply_scrivener_source_selection(folder);
                }
            }

            if browse_source_file {
                if let Some(file) = rfd::FileDialog::new()
                    .set_title("Select Scrivener (.scrivx) Manifest File")
                    .add_filter("Scrivener Project (*.scrivx)", &["scrivx"])
                    .pick_file()
                {
                    self.apply_scrivener_source_selection(file);
                }
            }

            if browse_dest_folder {
                if let Some(folder) = rfd::FileDialog::new()
                    .set_title("Select Destination Folder for Ported Project")
                    .pick_folder()
                {
                    self.scriv_dest_path = Some(folder);
                }
            }

            if run_import {
                if let (Some(src), Some(dst)) = (
                    self.scriv_source_path.clone(),
                    self.scriv_dest_path.clone(),
                ) {
                    let title_opt = if self.scriv_title_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.scriv_title_input.trim().to_string())
                    };
                    let author_opt = if self.scriv_author_input.trim().is_empty() {
                        None
                    } else {
                        Some(self.scriv_author_input.trim().to_string())
                    };

                    match self.import_scrivener_project(
                        &src,
                        &dst,
                        title_opt.as_deref(),
                        author_opt.as_deref(),
                    ) {
                        Ok(()) => {
                            close_modal = true;
                        }
                        Err(e) => {
                            self.set_status(format!("Scrivener import failed: {}", e));
                        }
                    }
                }
            }

            if close_modal {
                self.show_scrivener_import_modal = false;
            }
        }
    }
}

/// Configures Scrivener 3's signature crisp light theme and loads Windows Serif fonts (Sitka / Georgia).
fn configure_scrivener_theme_and_fonts(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = SCRIV_CANVAS_BG;
    visuals.window_fill = egui::Color32::from_rgb(248, 250, 248);
    visuals.faint_bg_color = SCRIV_BINDER_BG;
    visuals.extreme_bg_color = egui::Color32::WHITE;
    visuals.selection.bg_fill = egui::Color32::from_rgb(205, 226, 212);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, SCRIV_TEXT_DARK);
    visuals.widgets.noninteractive.fg_stroke.color = SCRIV_TEXT_DARK;
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(252, 253, 252);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, SCRIV_BORDER);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(234, 242, 236);
    visuals.widgets.active.bg_fill = SCRIV_BINDER_SELECTED_BG;
    ctx.set_visuals(visuals);

    // Load Windows system Serif font (Georgia or Times New Roman) for literary Sitka/Serif manuscript feel
    let mut fonts = egui::FontDefinitions::default();
    let serif_candidates = [
        r"C:\Windows\Fonts\georgia.ttf",
        r"C:\Windows\Fonts\times.ttf",
    ];
    let mut loaded_serif = false;
    for path in serif_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "system_serif".to_owned(),
                Arc::new(egui::FontData::from_owned(bytes)),
            );
            loaded_serif = true;
            break;
        }
    }

    let mut serif_list = Vec::new();
    if loaded_serif {
        serif_list.push("system_serif".to_owned());
    }
    if let Some(prop) = fonts.families.get(&egui::FontFamily::Proportional) {
        serif_list.extend(prop.iter().cloned());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name("Serif".into()), serif_list);

    ctx.set_fonts(fonts);
}

/// Formats an integer with thousands separators (e.g. `2458` -> `"2,458"`).
fn format_number_with_commas(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

/// Helper function to convert char offset to byte offset.
fn char_idx_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or(s.len())
}

/// Highlight markdown headings, bold (**bold**), and italic (*italic*) text in the editor.
/// Preserves 100% of characters so cursor navigation and text selections remain pixel-perfect.
fn highlight_markdown(
    ui: &egui::Ui,
    text: &str,
    wrap_width: f32,
    font_family: egui::FontFamily,
    base_size: f32,
    line_spacing: f32,
    show_markdown: bool,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = wrap_width;

    let text_color = SCRIV_TEXT_DARK;
    let weak_color = egui::Color32::from_rgb(140, 158, 146);
    let heading_color = egui::Color32::from_rgb(22, 78, 45); // Deep rich bookbinder green for #, ##, ###
    let bold_color = egui::Color32::from_rgb(18, 24, 20);
    let italic_color = egui::Color32::from_rgb(45, 55, 48);

    let extra_letter_spacing = if line_spacing > 1.05 { 0.1 } else { 0.0 };

    let normal_format = egui::TextFormat {
        font_id: egui::FontId::new(base_size, font_family.clone()),
        color: text_color,
        extra_letter_spacing,
        ..Default::default()
    };
    let tag_format = if show_markdown {
        egui::TextFormat {
            font_id: egui::FontId::new(base_size * 0.88, font_family.clone()),
            color: weak_color,
            ..Default::default()
        }
    } else {
        egui::TextFormat {
            font_id: egui::FontId::new(0.0, font_family.clone()),
            color: egui::Color32::TRANSPARENT,
            ..Default::default()
        }
    };

    for (line_idx, line) in text.split('\n').enumerate() {
        if line_idx > 0 {
            job.append("\n", 0.0, normal_format.clone());
        }

        if let Some(rest) = line.strip_prefix("# ") {
            job.append("# ", 0.0, tag_format.clone());
            job.append(
                rest,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::new(base_size * 1.4, font_family.clone()),
                    color: heading_color,
                    ..Default::default()
                },
            );
        } else if let Some(rest) = line.strip_prefix("## ") {
            job.append("## ", 0.0, tag_format.clone());
            job.append(
                rest,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::new(base_size * 1.2, font_family.clone()),
                    color: heading_color,
                    ..Default::default()
                },
            );
        } else if let Some(rest) = line.strip_prefix("### ") {
            job.append("### ", 0.0, tag_format.clone());
            job.append(
                rest,
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::new(base_size * 1.1, font_family.clone()),
                    color: heading_color,
                    ..Default::default()
                },
            );
        } else {
            highlight_inline_spans(
                line,
                &mut job,
                &font_family,
                base_size,
                &normal_format,
                &tag_format,
                bold_color,
                italic_color,
            );
        }
    }

    ui.fonts(|f| f.layout_job(job))
}

fn highlight_inline_spans(
    line: &str,
    job: &mut egui::text::LayoutJob,
    font_family: &egui::FontFamily,
    base_size: f32,
    normal_format: &egui::TextFormat,
    tag_format: &egui::TextFormat,
    bold_color: egui::Color32,
    italic_color: egui::Color32,
) {
    let mut remainder = line;

    while !remainder.is_empty() {
        let next_bold = remainder.find("**");
        let next_italic = remainder.find('*');
        let next_underline = remainder.find("<u>");

        let mut candidates = Vec::new();
        if let Some(b) = next_bold {
            candidates.push((b, "bold"));
        }
        if let Some(i) = next_italic {
            if Some(i) != next_bold {
                candidates.push((i, "italic"));
            }
        }
        if let Some(u) = next_underline {
            candidates.push((u, "underline"));
        }

        candidates.sort_by_key(|&(pos, _)| pos);

        if let Some(&(first_pos, kind)) = candidates.first() {
            match kind {
                "underline" => {
                    if let Some(close_idx) = remainder[first_pos + 3..].find("</u>") {
                        let actual_close = first_pos + 3 + close_idx;
                        if first_pos > 0 {
                            job.append(&remainder[..first_pos], 0.0, normal_format.clone());
                        }
                        job.append("<u>", 0.0, tag_format.clone());
                        let u_text = &remainder[first_pos + 3..actual_close];
                        job.append(
                            u_text,
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::new(base_size, font_family.clone()),
                                color: normal_format.color,
                                underline: egui::Stroke::new(1.0_f32, normal_format.color),
                                ..Default::default()
                            },
                        );
                        job.append("</u>", 0.0, tag_format.clone());
                        remainder = &remainder[actual_close + 4..];
                        continue;
                    }
                }
                "bold" => {
                    if let Some(close_idx) = remainder[first_pos + 2..].find("**") {
                        let actual_close = first_pos + 2 + close_idx;
                        if first_pos > 0 {
                            job.append(&remainder[..first_pos], 0.0, normal_format.clone());
                        }
                        job.append("**", 0.0, tag_format.clone());
                        let bold_text = &remainder[first_pos + 2..actual_close];
                        job.append(
                            bold_text,
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::new(base_size * 1.04, font_family.clone()),
                                color: bold_color,
                                ..Default::default()
                            },
                        );
                        job.append("**", 0.0, tag_format.clone());
                        remainder = &remainder[actual_close + 2..];
                        continue;
                    }
                }
                "italic" => {
                    if let Some(close_idx) = remainder[first_pos + 1..].find('*') {
                        let actual_close = first_pos + 1 + close_idx;
                        if first_pos > 0 {
                            job.append(&remainder[..first_pos], 0.0, normal_format.clone());
                        }
                        job.append("*", 0.0, tag_format.clone());
                        let italic_text = &remainder[first_pos + 1..actual_close];
                        job.append(
                            italic_text,
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::new(base_size, font_family.clone()),
                                color: italic_color,
                                italics: true,
                                ..Default::default()
                            },
                        );
                        job.append("*", 0.0, tag_format.clone());
                        remainder = &remainder[actual_close + 1..];
                        continue;
                    }
                }
                _ => {}
            }
        }

        job.append(remainder, 0.0, normal_format.clone());
        break;
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_auto_save_on_switching_scenes() {
        let temp_dir = std::env::temp_dir().join("test_auto_save_app");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut app = BookApp::default();
        let proj = BookProject::create_novel_blueprint(&temp_dir, "AutoSave Test", "Author")
            .expect("create blueprint");
        app.set_project(proj);

        app.select_scene(0, 0);
        let sc1_path = app.current_file_path.clone().expect("scene 1 path");

        app.editor_content = "# Scene 1\n\nModified draft content that must auto-save.".to_string();
        app.is_dirty = true;

        app.add_scene();

        let disk_content = fs::read_to_string(&sc1_path).expect("read sc1");
        assert_eq!(
            disk_content,
            "# Scene 1\n\nModified draft content that must auto-save."
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_auto_save_on_drop() {
        let temp_dir = std::env::temp_dir().join("test_auto_save_drop");
        let _ = fs::remove_dir_all(&temp_dir);

        let proj = BookProject::create_novel_blueprint(&temp_dir, "Drop Test", "Author")
            .expect("create blueprint");

        let file_path = temp_dir.join("manuscript").join("ch01").join("scene01.md");

        {
            let mut app = BookApp::default();
            app.set_project(proj);
            app.select_scene(0, 0);
            app.editor_content = "Unsaved content before window close".to_string();
            app.is_dirty = true;
        }

        let disk_content = fs::read_to_string(&file_path).expect("read file");
        assert_eq!(disk_content, "Unsaved content before window close");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_load_active_scene_file_from_disk() {
        let temp_dir = std::env::temp_dir().join("test_load_active_scene");
        let _ = fs::remove_dir_all(&temp_dir);

        let proj = BookProject::create_novel_blueprint(&temp_dir, "Load Test", "Author")
            .expect("create blueprint");

        let file_path = temp_dir.join("manuscript").join("ch01").join("scene01.md");
        fs::write(&file_path, "Content written directly to file system.").expect("write file");

        let mut app = BookApp::default();
        app.set_project(proj);
        app.select_scene(0, 0);

        assert_eq!(
            app.editor_content,
            "Content written directly to file system."
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_format_wrapper_with_selection() {
        let ctx = egui::Context::default();
        let mut app = BookApp::default();
        app.editor_content = "The quick brown fox".to_string();

        let editor_id = egui::Id::new("central_editor_text");
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(10),
            egui::text::CCursor::new(15),
        )));
        state.clone().store(&ctx, editor_id);

        app.apply_format_wrapper(&ctx, "**", "**");
        assert_eq!(app.editor_content, "The quick **brown** fox");

        app.apply_format_wrapper(&ctx, "**", "**");
        assert_eq!(app.editor_content, "The quick brown fox");

        let mut state = egui::text_edit::TextEditState::load(&ctx, editor_id).unwrap();
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(16),
            egui::text::CCursor::new(19),
        )));
        state.clone().store(&ctx, editor_id);

        app.apply_format_wrapper(&ctx, "*", "*");
        assert_eq!(app.editor_content, "The quick brown *fox*");

        // Test underline formatting wrapper with <u>
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(4),
            egui::text::CCursor::new(9),
        )));
        state.clone().store(&ctx, editor_id);
        app.apply_format_wrapper(&ctx, "<u>", "</u>");
        assert_eq!(app.editor_content, "The <u>quick</u> brown *fox*");

        // Test unwrapping <u>
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(4),
            egui::text::CCursor::new(16),
        )));
        state.store(&ctx, editor_id);
        app.apply_format_wrapper(&ctx, "<u>", "</u>");
        assert_eq!(app.editor_content, "The quick brown *fox*");
    }

    #[test]
    fn test_app_delete_chapter_and_scene() {
        let temp_dir = std::env::temp_dir().join("test_app_delete_ops");
        let _ = fs::remove_dir_all(&temp_dir);

        let proj = BookProject::create_novel_blueprint(&temp_dir, "Delete Test", "Author")
            .expect("create blueprint");

        let mut app = BookApp::default();
        app.set_project(proj);

        app.add_scene();
        assert_eq!(app.project.as_ref().unwrap().chapters[0].scenes.len(), 2);

        app.delete_scene(0, 1);
        assert_eq!(app.project.as_ref().unwrap().chapters[0].scenes.len(), 1);

        app.add_chapter();
        assert_eq!(app.project.as_ref().unwrap().chapters.len(), 2);

        app.delete_chapter(0);
        assert_eq!(app.project.as_ref().unwrap().chapters.len(), 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_app_update_project_metadata() {
        let temp_dir = std::env::temp_dir().join("test_app_metadata_edit");
        let _ = fs::remove_dir_all(&temp_dir);

        let proj = BookProject::create_novel_blueprint(&temp_dir, "Draft One", "Jane Writer")
            .expect("create blueprint");

        let mut app = BookApp::default();
        app.set_project(proj);

        app.open_project_metadata_modal();
        assert!(app.show_project_metadata_modal);
        assert_eq!(app.edit_project_title_input, "Draft One");
        assert_eq!(app.edit_project_author_input, "Jane Writer");

        // Simulate user editing title and author
        app.edit_project_title_input = "Revised Title".to_string();
        app.edit_project_author_input = "J. R. Writer".to_string();

        app.project
            .as_mut()
            .unwrap()
            .update_project_metadata(&app.edit_project_title_input, &app.edit_project_author_input)
            .unwrap();

        assert_eq!(app.project.as_ref().unwrap().title, "Revised Title");
        assert_eq!(app.project.as_ref().unwrap().author, "J. R. Writer");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_app_import_scrivener_project() {
        let scriv_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("READ-ONLY")
            .join("The Traveller.scriv");
        if !scriv_dir.exists() {
            return;
        }

        let temp_dest = std::env::temp_dir().join("test_app_scriv_import");
        let _ = fs::remove_dir_all(&temp_dest);

        let mut app = BookApp::default();
        app.import_scrivener_project(&scriv_dir, &temp_dest, None, None)
            .expect("import Scrivener project into BookApp");

        let proj = app.project.as_ref().expect("project loaded");
        assert_eq!(proj.title, "The Traveller");
        assert_eq!(proj.chapters.len(), 11);
        assert!(app.editor_content.starts_with("7 A.M. sharp"));

        let _ = fs::remove_dir_all(&temp_dest);
    }

    #[test]
    fn test_markdown_in_editor_toggle_and_layouter() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let text = "# Chapter 1\n\nThis is **bold**, *italic*, and <u>underlined</u> text.";
                
                // When markdown_in_editor is true (default)
                let galley_with_md = highlight_markdown(
                    ui,
                    text,
                    600.0,
                    egui::FontFamily::Proportional,
                    14.0,
                    1.0,
                    true,
                );
                assert!(galley_with_md.size().x > 0.0);

                // When markdown_in_editor is false (hidden markdown markup)
                let galley_hidden_md = highlight_markdown(
                    ui,
                    text,
                    600.0,
                    egui::FontFamily::Proportional,
                    14.0,
                    1.0,
                    false,
                );
                assert!(galley_hidden_md.size().x > 0.0);
            });
        });
    }

    #[test]
    fn test_app_reorder_chapters_ui() {
        let temp_dir = std::env::temp_dir().join("test_app_reorder_ch_ui");
        let _ = fs::remove_dir_all(&temp_dir);

        let proj = BookProject::create_novel_blueprint(&temp_dir, "Reorder UI Test", "Author")
            .expect("create blueprint");

        let mut app = BookApp::default();
        app.set_project(proj);

        // Add a second and third chapter
        app.add_chapter();
        app.add_chapter();
        assert_eq!(app.project.as_ref().unwrap().chapters.len(), 3);

        let original_first = app.project.as_ref().unwrap().chapters[0].name.clone();
        let original_third = app.project.as_ref().unwrap().chapters[2].name.clone();

        // Reorder chapter 0 to index 2
        app.reorder_chapter_ui(0, 2);

        let p = app.project.as_ref().unwrap();
        assert_eq!(p.chapters[2].name, original_first);
        assert_eq!(p.chapters[1].name, original_third);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
