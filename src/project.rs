use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

fn default_version() -> String {
    "1.0.2".to_string()
}

/// Represents the type of a chapter: a numbered regular chapter or a specialized unnumbered section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChapterKind {
    #[default]
    Numbered,
    Specialized(String),
}

impl ChapterKind {
    pub fn is_numbered(&self) -> bool {
        matches!(self, ChapterKind::Numbered)
    }

    pub fn preset_options() -> &'static [&'static str] {
        &[
            "Prologue",
            "Epilogue",
            "Writer's Note",
            "Author's Note",
            "Foreword",
            "Preface",
            "Introduction",
            "Interlude",
            "Afterword",
            "Acknowledgments",
        ]
    }
}

/// Persisted metadata for an individual chapter in `book.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ChapterMeta {
    /// Writer-facing display title in the sidebar (does not affect numbered chapter titles in print).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writer_title: Option<String>,
    /// If `Some(title)`, this chapter is a specialized chapter (e.g., "Prologue") that does not increment
    /// chapter numbers and uses this unique title in the final compiled PDF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_title: Option<String>,
    /// Writer-facing display titles for scenes inside this chapter, keyed by filename (e.g. `scene01.md`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub scene_titles: HashMap<String, String>,
    /// Optional explicit ordering of scene filenames inside this chapter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scene_order: Vec<String>,
}

/// Metadata describing a book project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookMetadata {
    pub title: String,
    pub author: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional per-chapter metadata keyed by chapter directory name (e.g. `ch01`, `sp01_prologue`).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub chapter_meta: HashMap<String, ChapterMeta>,
    /// Optional explicit ordering of chapter directory names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapter_order: Vec<String>,
}

impl BookMetadata {
    pub fn new(title: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            author: author.into(),
            version: default_version(),
            language: Some("en".to_string()),
            description: None,
            chapter_meta: HashMap::new(),
            chapter_order: Vec::new(),
        }
    }
}

/// Top-level book configuration structure written to and read from `book.toml`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BookConfig {
    pub book: BookMetadata,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BookConfigRaw {
    Nested { book: BookMetadata },
    Flat(BookMetadata),
}

impl<'de> Deserialize<'de> for BookConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = BookConfigRaw::deserialize(deserializer)?;
        match raw {
            BookConfigRaw::Nested { book } => Ok(Self { book }),
            BookConfigRaw::Flat(book) => Ok(Self { book }),
        }
    }
}

impl std::ops::Deref for BookConfig {
    type Target = BookMetadata;

    fn deref(&self) -> &Self::Target {
        &self.book
    }
}

impl std::ops::DerefMut for BookConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.book
    }
}

impl BookConfig {
    pub fn new(title: impl Into<String>, author: impl Into<String>) -> Self {
        Self {
            book: BookMetadata::new(title, author),
        }
    }
}

/// Represents an individual Markdown scene or front-matter document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneFile {
    pub path: PathBuf,
    pub name: String,
    /// Writer-only display title shown in the app sidebar/breadcrumbs.
    pub title: String,
}

impl SceneFile {
    pub fn new(path: impl Into<PathBuf>, title: impl Into<String>) -> Self {
        let path = path.into();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        Self {
            path,
            name,
            title: title.into(),
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&name);
        let title = humanize_title(stem);
        Self { path, name, title }
    }

    pub fn read_content(&self) -> Result<String> {
        fs::read_to_string(&self.path)
            .with_context(|| format!("Failed to read content from {:?}", self.path))
    }

    pub fn write_content(&self, content: &str) -> Result<()> {
        fs::write(&self.path, content)
            .with_context(|| format!("Failed to write content to {:?}", self.path))
    }
}

/// Represents a chapter folder inside the manuscript directory (e.g. `ch01` or `sp01_prologue`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChapterFolder {
    pub path: PathBuf,
    pub name: String,
    /// Writer-only display name shown in the sidebar and breadcrumbs.
    pub title: String,
    /// Determines whether this chapter increments the chapter number or is a specialized chapter.
    #[serde(default)]
    pub kind: ChapterKind,
    pub scenes: Vec<SceneFile>,
}

impl ChapterFolder {
    pub fn new(path: impl Into<PathBuf>, title: impl Into<String>) -> Self {
        let path = path.into();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        Self {
            path,
            name,
            title: title.into(),
            kind: ChapterKind::Numbered,
            scenes: Vec::new(),
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let title = humanize_chapter_title(&name);
        let mut chapter = Self {
            path: path.clone(),
            name,
            title,
            kind: ChapterKind::Numbered,
            scenes: Vec::new(),
        };
        chapter.load_scenes()?;
        Ok(chapter)
    }

    pub fn load_scenes(&mut self) -> Result<()> {
        self.scenes.clear();
        if !self.path.exists() {
            return Ok(());
        }
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.path)
            .with_context(|| format!("Failed to read directory {:?}", self.path))?
        {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                        entries.push(entry_path);
                    }
                }
            }
        }
        entries.sort();
        for file_path in entries {
            self.scenes.push(SceneFile::from_path(file_path));
        }
        Ok(())
    }

    pub fn add_scene(&mut self, filename: &str, title: Option<&str>, content: &str) -> Result<SceneFile> {
        let scene_path = self.path.join(filename);
        fs::write(&scene_path, content)
            .with_context(|| format!("Failed to write scene file at {:?}", scene_path))?;
        let scene = if let Some(t) = title {
            SceneFile::new(&scene_path, t)
        } else {
            SceneFile::from_path(&scene_path)
        };
        self.scenes.push(scene.clone());
        Ok(scene)
    }

    pub fn is_specialized(&self) -> bool {
        matches!(self.kind, ChapterKind::Specialized(_))
    }

    pub fn specialized_print_title(&self) -> Option<&str> {
        match &self.kind {
            ChapterKind::Specialized(t) => Some(t.as_str()),
            ChapterKind::Numbered => None,
        }
    }
}

/// Represents the overall book project workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookProject {
    pub root_dir: PathBuf,
    pub title: String,
    pub author: String,
    pub config: BookConfig,
    pub front_matter: Vec<SceneFile>,
    pub chapters: Vec<ChapterFolder>,
}

impl BookProject {
    /// Initializes a novel blueprint with `book.toml`, `front_matter/` (Title Page, Copyright, Dedication),
    /// and `manuscript/ch01/` with a clean starter scene file.
    pub fn create_novel_blueprint<P: AsRef<Path>>(
        root_dir: P,
        title: &str,
        author: &str,
    ) -> Result<Self> {
        let root = root_dir.as_ref().to_path_buf();
        fs::create_dir_all(&root)
            .with_context(|| format!("Failed to create project root directory at {:?}", root))?;

        // 1. Initialize book.toml
        let mut metadata = BookMetadata {
            title: title.to_string(),
            author: author.to_string(),
            version: default_version(),
            language: Some("en".to_string()),
            description: None,
            chapter_meta: HashMap::new(),
            chapter_order: vec!["ch01".to_string()],
        };
        metadata.chapter_meta.insert(
            "ch01".to_string(),
            ChapterMeta {
                writer_title: Some("Chapter 1".to_string()),
                special_title: None,
                scene_titles: HashMap::from([("scene01.md".to_string(), "Scene 1".to_string())]),
                scene_order: vec!["scene01.md".to_string()],
            },
        );

        let config = BookConfig { book: metadata };
        let toml_string = toml::to_string_pretty(&config)
            .context("Failed to serialize book.toml configuration")?;
        let book_toml_path = root.join("book.toml");
        fs::write(&book_toml_path, toml_string)
            .with_context(|| format!("Failed to write book.toml at {:?}", book_toml_path))?;

        // 2. Create front_matter/ (Title Page, Copyright, Dedication)
        let front_matter_dir = root.join("front_matter");
        fs::create_dir_all(&front_matter_dir)
            .with_context(|| format!("Failed to create front_matter directory at {:?}", front_matter_dir))?;

        let title_page_content = "# $Title\n\n### By $Author\n\n*Draft Edition*\n".to_string();
        let copyright_content = "# Copyright\n\nCopyright © 2026 by $Author.\n\nAll rights reserved. No part of this publication may be reproduced, distributed, or transmitted in any form or by any means without the prior written permission of the author.\n".to_string();
        let dedication_content = "# Dedication\n\n*Dedicated to all readers and dreamers.*\n";

        let title_page_file = front_matter_dir.join("title_page.md");
        let copyright_file = front_matter_dir.join("copyright.md");
        let dedication_file = front_matter_dir.join("dedication.md");

        fs::write(&title_page_file, title_page_content)
            .with_context(|| format!("Failed to write {:?}", title_page_file))?;
        fs::write(&copyright_file, copyright_content)
            .with_context(|| format!("Failed to write {:?}", copyright_file))?;
        fs::write(&dedication_file, dedication_content)
            .with_context(|| format!("Failed to write {:?}", dedication_file))?;

        let front_matter = vec![
            SceneFile::new(&title_page_file, "Title Page"),
            SceneFile::new(&copyright_file, "Copyright"),
            SceneFile::new(&dedication_file, "Dedication"),
        ];

        // 3. Create manuscript/ch01/ with completely blank scene01.md
        let manuscript_dir = root.join("manuscript");
        let ch01_dir = manuscript_dir.join("ch01");
        fs::create_dir_all(&ch01_dir)
            .with_context(|| format!("Failed to create manuscript chapter directory at {:?}", ch01_dir))?;

        let scene01_file = ch01_dir.join("scene01.md");
        fs::write(&scene01_file, "")
            .with_context(|| format!("Failed to write {:?}", scene01_file))?;

        let scene01 = SceneFile::new(&scene01_file, "Scene 1");
        let ch01 = ChapterFolder {
            path: ch01_dir,
            name: "ch01".to_string(),
            title: "Chapter 1".to_string(),
            kind: ChapterKind::Numbered,
            scenes: vec![scene01],
        };

        Ok(Self {
            root_dir: root,
            title: title.to_string(),
            author: author.to_string(),
            config,
            front_matter,
            chapters: vec![ch01],
        })
    }

    /// Opens and loads an existing book project from `root_dir`.
    pub fn open<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        let root = root_dir.as_ref().to_path_buf();
        let book_toml_path = root.join("book.toml");
        if !book_toml_path.exists() {
            return Err(anyhow!("book.toml not found in {:?}", root));
        }

        let toml_str = fs::read_to_string(&book_toml_path)
            .with_context(|| format!("Failed to read {:?}", book_toml_path))?;
        let config: BookConfig = toml::from_str(&toml_str)
            .with_context(|| format!("Failed to parse TOML in {:?}", book_toml_path))?;

        let title = config.title.clone();
        let author = config.author.clone();

        // Scan front_matter
        let mut front_matter = Vec::new();
        let front_matter_dir = root.join("front_matter");
        if front_matter_dir.exists() {
            let mut files = Vec::new();
            for entry in fs::read_dir(&front_matter_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                            files.push(path);
                        }
                    }
                }
            }
            files.sort();
            for f in files {
                front_matter.push(SceneFile::from_path(f));
            }
        }

        // Scan manuscript chapters
        let mut chapters_map: HashMap<String, ChapterFolder> = HashMap::new();
        let manuscript_dir = root.join("manuscript");
        let mut dir_names = Vec::new();

        if manuscript_dir.exists() {
            let mut dirs = Vec::new();
            for entry in fs::read_dir(&manuscript_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                }
            }
            dirs.sort();
            for d in dirs {
                let mut chapter = ChapterFolder::from_path(&d)?;
                // Apply saved metadata from book.toml if available
                if let Some(meta) = config.chapter_meta.get(&chapter.name) {
                    if let Some(wt) = &meta.writer_title {
                        chapter.title = wt.clone();
                    }
                    if let Some(st) = &meta.special_title {
                        chapter.kind = ChapterKind::Specialized(st.clone());
                        if meta.writer_title.is_none() {
                            chapter.title = st.clone();
                        }
                    }
                    for scene in &mut chapter.scenes {
                        if let Some(scene_title) = meta.scene_titles.get(&scene.name) {
                            scene.title = scene_title.clone();
                        }
                    }
                    if !meta.scene_order.is_empty() {
                        let mut scene_map: HashMap<String, SceneFile> =
                            chapter.scenes.drain(..).map(|s| (s.name.clone(), s)).collect();
                        for sname in &meta.scene_order {
                            if let Some(s) = scene_map.remove(sname) {
                                chapter.scenes.push(s);
                            }
                        }
                        let mut remaining: Vec<_> = scene_map.into_values().collect();
                        remaining.sort_by(|a, b| a.name.cmp(&b.name));
                        chapter.scenes.extend(remaining);
                    }
                }
                dir_names.push(chapter.name.clone());
                chapters_map.insert(chapter.name.clone(), chapter);
            }
        }

        // Respect chapter_order if present, appending any unlisted directories at the end
        let mut chapters = Vec::new();
        for ordered_name in &config.chapter_order {
            if let Some(ch) = chapters_map.remove(ordered_name) {
                chapters.push(ch);
            }
        }
        for name in dir_names {
            if let Some(ch) = chapters_map.remove(&name) {
                chapters.push(ch);
            }
        }

        let mut project = Self {
            root_dir: root,
            title,
            author,
            config,
            front_matter,
            chapters,
        };
        project.sync_metadata();
        Ok(project)
    }

    /// Alias for `open`.
    pub fn load<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        Self::open(root_dir)
    }

    /// Synchronizes in-memory title, author, chapters and scene writer titles into `self.config`.
    pub fn sync_metadata(&mut self) {
        self.config.book.title = self.title.clone();
        self.config.book.author = self.author.clone();
        self.config.chapter_order = self.chapters.iter().map(|c| c.name.clone()).collect();
        let mut new_meta = HashMap::new();
        for ch in &self.chapters {
            let mut scene_titles = HashMap::new();
            let mut scene_order = Vec::new();
            for sc in &ch.scenes {
                scene_titles.insert(sc.name.clone(), sc.title.clone());
                scene_order.push(sc.name.clone());
            }
            let special_title = match &ch.kind {
                ChapterKind::Numbered => None,
                ChapterKind::Specialized(s) => Some(s.clone()),
            };
            new_meta.insert(
                ch.name.clone(),
                ChapterMeta {
                    writer_title: Some(ch.title.clone()),
                    special_title,
                    scene_titles,
                    scene_order,
                },
            );
        }
        self.config.chapter_meta = new_meta;
    }

    /// Updates the book title and author name for the entire project, and saves to `book.toml`.
    pub fn update_project_metadata(&mut self, title: &str, author: &str) -> Result<()> {
        let t = title.trim();
        let a = author.trim();
        if t.is_empty() {
            return Err(anyhow!("Book title cannot be empty"));
        }
        if a.is_empty() {
            return Err(anyhow!("Author name cannot be empty"));
        }
        self.title = t.to_string();
        self.author = a.to_string();
        self.save_config()
    }

    /// Saves the current configuration (including custom writer names & specialized chapter metadata) to `book.toml`.
    pub fn save_config(&mut self) -> Result<()> {
        self.sync_metadata();
        let toml_str = toml::to_string_pretty(&self.config)
            .context("Failed to serialize book.toml configuration")?;
        let path = self.root_dir.join("book.toml");
        fs::write(&path, toml_str)
            .with_context(|| format!("Failed to write book.toml at {:?}", path))?;
        Ok(())
    }

    pub fn front_matter_dir(&self) -> PathBuf {
        self.root_dir.join("front_matter")
    }

    pub fn manuscript_dir(&self) -> PathBuf {
        self.root_dir.join("manuscript")
    }

    /// Adds a new chapter folder inside `manuscript/`.
    pub fn add_chapter(&mut self, chapter_name: &str) -> Result<&mut ChapterFolder> {
        let chapter_path = self.manuscript_dir().join(chapter_name);
        fs::create_dir_all(&chapter_path)
            .with_context(|| format!("Failed to create chapter directory {:?}", chapter_path))?;
        let chapter = ChapterFolder::from_path(chapter_path)?;
        self.chapters.push(chapter);
        self.save_config()?;
        Ok(self.chapters.last_mut().unwrap())
    }

    /// Automatically creates the next regular numbered chapter (e.g. `ch02`) with a completely blank scene.
    pub fn create_next_chapter(&mut self) -> Result<usize> {
        // Find a unique directory name chXX
        let mut idx = 1;
        while self.manuscript_dir().join(format!("ch{:02}", idx)).exists()
            || self.chapters.iter().any(|c| c.name == format!("ch{:02}", idx))
        {
            idx += 1;
        }
        let chapter_name = format!("ch{:02}", idx);
        let chapter_dir = self.manuscript_dir().join(&chapter_name);
        fs::create_dir_all(&chapter_dir)
            .with_context(|| format!("Failed to create chapter directory {:?}", chapter_dir))?;

        let numbered_count = self.chapters.iter().filter(|c| c.kind.is_numbered()).count() + 1;
        let scene_file = chapter_dir.join("scene01.md");
        // Open completely blank as requested
        fs::write(&scene_file, "")
            .with_context(|| format!("Failed to write starter scene {:?}", scene_file))?;

        let mut chapter = ChapterFolder::from_path(&chapter_dir)?;
        chapter.title = format!("Chapter {}", numbered_count);
        chapter.kind = ChapterKind::Numbered;
        self.chapters.push(chapter);
        self.save_config()?;
        Ok(self.chapters.len() - 1)
    }

    /// Creates a specialized chapter (such as Prologue, Epilogue, Writer's Note) that does not
    /// increment the chapter counter, displays its unique title in the final PDF, and opens with a completely blank scene.
    /// If `insert_at_beginning` is true (e.g. for a Prologue), inserts it at index 0.
    pub fn create_specialized_chapter(
        &mut self,
        special_title: &str,
        insert_at_beginning: bool,
    ) -> Result<usize> {
        let slug: String = special_title
            .chars()
            .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
            .collect();
        let slug = slug.trim_matches('_').to_string();
        let slug = if slug.is_empty() { "special".to_string() } else { slug };

        let mut idx = 1;
        let mut folder_name = format!("sp{:02}_{}", idx, slug);
        while self.manuscript_dir().join(&folder_name).exists()
            || self.chapters.iter().any(|c| c.name == folder_name)
        {
            idx += 1;
            folder_name = format!("sp{:02}_{}", idx, slug);
        }

        let chapter_dir = self.manuscript_dir().join(&folder_name);
        fs::create_dir_all(&chapter_dir)
            .with_context(|| format!("Failed to create specialized chapter directory {:?}", chapter_dir))?;

        let scene_file = chapter_dir.join("scene01.md");
        // Open completely blank as requested
        fs::write(&scene_file, "")
            .with_context(|| format!("Failed to write starter scene {:?}", scene_file))?;

        let mut chapter = ChapterFolder::from_path(&chapter_dir)?;
        chapter.title = special_title.to_string();
        chapter.kind = ChapterKind::Specialized(special_title.to_string());

        let inserted_idx = if insert_at_beginning {
            self.chapters.insert(0, chapter);
            0
        } else {
            self.chapters.push(chapter);
            self.chapters.len() - 1
        };

        self.save_config()?;
        Ok(inserted_idx)
    }

    /// Automatically creates the next scene in the specified chapter with a completely blank file.
    pub fn create_next_scene(&mut self, chapter_idx: usize) -> Result<usize> {
        if chapter_idx >= self.chapters.len() {
            return Err(anyhow!("Invalid chapter index {}", chapter_idx));
        }
        let chapter = &mut self.chapters[chapter_idx];
        let mut next_scene_num = chapter.scenes.len() + 1;
        while chapter.path.join(format!("scene{:02}.md", next_scene_num)).exists()
            || chapter.scenes.iter().any(|s| s.name == format!("scene{:02}.md", next_scene_num))
        {
            next_scene_num += 1;
        }
        let filename = format!("scene{:02}.md", next_scene_num);
        let scene_path = chapter.path.join(&filename);
        // Open completely blank as requested
        fs::write(&scene_path, "")
            .with_context(|| format!("Failed to write scene {:?}", scene_path))?;
        let display_num = chapter.scenes.len() + 1;
        let scene = SceneFile::new(&scene_path, format!("Scene {}", display_num));
        chapter.scenes.push(scene);
        let new_idx = chapter.scenes.len() - 1;
        self.save_config()?;
        Ok(new_idx)
    }

    /// Updates the writer-only display name of a chapter and persists it to `book.toml`.
    pub fn rename_chapter(&mut self, chapter_idx: usize, new_writer_title: &str) -> Result<()> {
        if chapter_idx >= self.chapters.len() {
            return Err(anyhow!("Invalid chapter index {}", chapter_idx));
        }
        let trimmed = new_writer_title.trim();
        if !trimmed.is_empty() {
            self.chapters[chapter_idx].title = trimmed.to_string();
            self.save_config()?;
        }
        Ok(())
    }

    /// Updates the writer-only display name of a scene and persists it to `book.toml`.
    pub fn rename_scene(
        &mut self,
        chapter_idx: usize,
        scene_idx: usize,
        new_writer_title: &str,
    ) -> Result<()> {
        if chapter_idx >= self.chapters.len() {
            return Err(anyhow!("Invalid chapter index {}", chapter_idx));
        }
        if scene_idx >= self.chapters[chapter_idx].scenes.len() {
            return Err(anyhow!("Invalid scene index {}", scene_idx));
        }
        let trimmed = new_writer_title.trim();
        if !trimmed.is_empty() {
            self.chapters[chapter_idx].scenes[scene_idx].title = trimmed.to_string();
            self.save_config()?;
        }
        Ok(())
    }

    /// Deletes a chapter and its directory from disk.
    pub fn delete_chapter(&mut self, chapter_idx: usize) -> Result<()> {
        if chapter_idx >= self.chapters.len() {
            return Err(anyhow!("Invalid chapter index {}", chapter_idx));
        }
        let removed = self.chapters.remove(chapter_idx);
        if removed.path.exists() {
            fs::remove_dir_all(&removed.path)
                .with_context(|| format!("Failed to remove chapter directory {:?}", removed.path))?;
        }
        self.save_config()?;
        Ok(())
    }

    /// Deletes a scene file from a chapter and removes it from disk.
    pub fn delete_scene(&mut self, chapter_idx: usize, scene_idx: usize) -> Result<()> {
        if chapter_idx >= self.chapters.len() {
            return Err(anyhow!("Invalid chapter index {}", chapter_idx));
        }
        if scene_idx >= self.chapters[chapter_idx].scenes.len() {
            return Err(anyhow!("Invalid scene index {}", scene_idx));
        }
        let removed = self.chapters[chapter_idx].scenes.remove(scene_idx);
        if removed.path.exists() {
            fs::remove_file(&removed.path)
                .with_context(|| format!("Failed to remove scene file {:?}", removed.path))?;
        }
        self.save_config()?;
        Ok(())
    }

    /// Reorders chapters by moving a chapter from `from_idx` to `to_idx` and saves the updated order to `book.toml`.
    pub fn reorder_chapter(&mut self, from_idx: usize, to_idx: usize) -> Result<()> {
        if from_idx >= self.chapters.len() || to_idx >= self.chapters.len() || from_idx == to_idx {
            return Ok(());
        }
        let ch = self.chapters.remove(from_idx);
        self.chapters.insert(to_idx, ch);
        self.save_config()
    }

    /// Reorders a scene, either within the same chapter or across different chapters, and persists to disk and `book.toml`.
    /// Returns the new `PathBuf` of the scene file.
    pub fn reorder_scene(
        &mut self,
        from_ch: usize,
        from_sc: usize,
        to_ch: usize,
        to_sc: usize,
    ) -> Result<PathBuf> {
        if from_ch >= self.chapters.len() {
            return Err(anyhow!("Invalid source chapter index {}", from_ch));
        }
        if from_sc >= self.chapters[from_ch].scenes.len() {
            return Err(anyhow!("Invalid source scene index {}", from_sc));
        }
        if to_ch >= self.chapters.len() {
            return Err(anyhow!("Invalid destination chapter index {}", to_ch));
        }

        if from_ch == to_ch {
            let num_scenes = self.chapters[from_ch].scenes.len();
            let target_sc = to_sc.min(num_scenes - 1);
            if from_sc != target_sc {
                let sc = self.chapters[from_ch].scenes.remove(from_sc);
                self.chapters[from_ch].scenes.insert(target_sc, sc);
                self.save_config()?;
            }
            Ok(self.chapters[from_ch].scenes[target_sc].path.clone())
        } else {
            let mut sc = self.chapters[from_ch].scenes.remove(from_sc);
            let dest_dir = self.chapters[to_ch].path.clone();

            // Find a unique filename in dest_dir
            let mut final_name = sc.name.clone();
            let mut final_path = dest_dir.join(&final_name);
            if final_path.exists() {
                let stem = Path::new(&sc.name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("scene");
                let ext = Path::new(&sc.name)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("md");
                let mut counter = 1;
                while final_path.exists() {
                    final_name = format!("{}_{:02}.{}", stem, counter, ext);
                    final_path = dest_dir.join(&final_name);
                    counter += 1;
                }
            }

            fs::rename(&sc.path, &final_path).with_context(|| {
                format!("Failed to move scene file {:?} to {:?}", sc.path, final_path)
            })?;

            sc.path = final_path.clone();
            sc.name = final_name;

            let target_sc = to_sc.min(self.chapters[to_ch].scenes.len());
            self.chapters[to_ch].scenes.insert(target_sc, sc);
            self.save_config()?;
            Ok(final_path)
        }
    }
}

fn humanize_title(stem: &str) -> String {
    let lower = stem.to_lowercase();
    if lower == "title_page" || lower == "title-page" || lower == "titlepage" {
        return "Title Page".to_string();
    }
    if lower == "copyright" {
        return "Copyright".to_string();
    }
    if lower == "dedication" {
        return "Dedication".to_string();
    }
    if lower.starts_with("scene") {
        let num_part = stem.trim_start_matches(|c: char| !c.is_numeric());
        if !num_part.is_empty() {
            if let Ok(n) = num_part.parse::<u32>() {
                return format!("Scene {}", n);
            }
        }
    }
    stem.replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn humanize_chapter_title(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.starts_with("ch") {
        let num_part = name.trim_start_matches(|c: char| !c.is_numeric());
        if !num_part.is_empty() {
            if let Ok(n) = num_part.parse::<u32>() {
                return format!("Chapter {}", n);
            }
        }
    }
    humanize_title(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_novel_blueprint() {
        let temp_dir = std::env::temp_dir().join("test_book_project_novel");
        let _ = fs::remove_dir_all(&temp_dir);

        let title = "The Chronicles of Rust";
        let author = "Ferris Crab";

        let mut project = BookProject::create_novel_blueprint(&temp_dir, title, author)
            .expect("create_novel_blueprint should succeed");

        assert_eq!(project.title, title);
        assert_eq!(project.author, author);
        assert_eq!(project.root_dir, temp_dir);

        // Verify book.toml
        let book_toml = temp_dir.join("book.toml");
        assert!(book_toml.exists(), "book.toml must exist");
        let toml_content = fs::read_to_string(&book_toml).expect("read book.toml");
        assert!(toml_content.contains(title));
        assert!(toml_content.contains(author));

        // Verify front_matter/
        let front_matter = temp_dir.join("front_matter");
        assert!(front_matter.exists(), "front_matter/ must exist");
        assert!(front_matter.join("title_page.md").exists(), "title_page.md must exist");
        assert!(front_matter.join("copyright.md").exists(), "copyright.md must exist");
        assert!(front_matter.join("dedication.md").exists(), "dedication.md must exist");

        assert_eq!(project.front_matter.len(), 3);

        // Verify manuscript/ch01/
        let ch01 = temp_dir.join("manuscript").join("ch01");
        assert!(ch01.exists(), "manuscript/ch01/ must exist");
        let scene01 = ch01.join("scene01.md");
        assert!(scene01.exists(), "scene01.md must exist");

        // Verify newly created scene is completely blank
        let scene01_content = fs::read_to_string(&scene01).expect("read scene01");
        assert_eq!(scene01_content, "", "Starter scene should be blank");

        assert_eq!(project.chapters.len(), 1);
        assert_eq!(project.chapters[0].name, "ch01");
        assert_eq!(project.chapters[0].scenes.len(), 1);
        assert_eq!(project.chapters[0].scenes[0].name, "scene01.md");

        // Test create_next_chapter and create_next_scene
        let ch2_idx = project.create_next_chapter().expect("create next chapter");
        assert_eq!(ch2_idx, 1);
        assert_eq!(project.chapters.len(), 2);
        assert_eq!(project.chapters[1].name, "ch02");
        let ch2_sc1 = project.chapters[1].scenes[0].read_content().unwrap();
        assert_eq!(ch2_sc1, "", "New chapter scene should be blank");

        let sc2_idx = project.create_next_scene(0).expect("create next scene in ch01");
        assert_eq!(sc2_idx, 1);
        assert_eq!(project.chapters[0].scenes.len(), 2);
        let ch1_sc2 = project.chapters[0].scenes[1].read_content().unwrap();
        assert_eq!(ch1_sc2, "", "New scene should be blank");

        // Verify BookProject::open reloads correctly
        let loaded = BookProject::open(&temp_dir).expect("BookProject::open should succeed");
        assert_eq!(loaded.title, title);
        assert_eq!(loaded.author, author);
        assert_eq!(loaded.front_matter.len(), 3);
        assert_eq!(loaded.chapters.len(), 2);
        assert_eq!(loaded.chapters[0].scenes.len(), 2);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_rename_delete_and_specialized_chapters() {
        let temp_dir = std::env::temp_dir().join("test_book_project_ops");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut project = BookProject::create_novel_blueprint(&temp_dir, "Ops Novel", "Author")
            .expect("create blueprint");

        // Rename chapter 1 and scene 1 for writer's view
        project.rename_chapter(0, "The Awakening (Draft Note)").unwrap();
        project.rename_scene(0, 0, "Opening Hook").unwrap();

        assert_eq!(project.chapters[0].title, "The Awakening (Draft Note)");
        assert_eq!(project.chapters[0].scenes[0].title, "Opening Hook");

        // Add specialized Prologue at beginning and Epilogue at end
        let prol_idx = project.create_specialized_chapter("Prologue", true).unwrap();
        assert_eq!(prol_idx, 0);
        assert!(project.chapters[0].is_specialized());
        assert_eq!(project.chapters[0].specialized_print_title(), Some("Prologue"));

        let epil_idx = project.create_specialized_chapter("Epilogue", false).unwrap();
        assert_eq!(epil_idx, 2);
        assert_eq!(project.chapters[2].specialized_print_title(), Some("Epilogue"));

        // Reopen from disk and check persistence of writer names, order, and specialized kinds
        let mut reopened = BookProject::open(&temp_dir).unwrap();
        assert_eq!(reopened.chapters.len(), 3);
        assert_eq!(reopened.chapters[0].specialized_print_title(), Some("Prologue"));
        assert_eq!(reopened.chapters[1].title, "The Awakening (Draft Note)");
        assert!(reopened.chapters[1].kind.is_numbered());
        assert_eq!(reopened.chapters[1].scenes[0].title, "Opening Hook");
        assert_eq!(reopened.chapters[2].specialized_print_title(), Some("Epilogue"));

        // Delete scene and chapter
        reopened.create_next_scene(1).unwrap();
        assert_eq!(reopened.chapters[1].scenes.len(), 2);
        reopened.delete_scene(1, 0).unwrap();
        assert_eq!(reopened.chapters[1].scenes.len(), 1);

        reopened.delete_chapter(0).unwrap(); // Delete Prologue
        assert_eq!(reopened.chapters.len(), 2);
        assert_eq!(reopened.chapters[0].title, "The Awakening (Draft Note)");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scene_read_write() {
        let temp_dir = std::env::temp_dir().join("test_scene_rw");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let file_path = temp_dir.join("scene01.md");
        let scene = SceneFile::new(&file_path, "Scene 1");
        scene.write_content("Hello World").unwrap();
        assert_eq!(scene.read_content().unwrap(), "Hello World");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_update_project_metadata_persistence() {
        let temp_dir = std::env::temp_dir().join("test_update_proj_metadata");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut project = BookProject::create_novel_blueprint(&temp_dir, "Original Title", "Original Author")
            .expect("create blueprint");

        assert_eq!(project.title, "Original Title");
        assert_eq!(project.author, "Original Author");

        project
            .update_project_metadata("Renamed Epic Title", "Jane Doe")
            .expect("update metadata");

        assert_eq!(project.title, "Renamed Epic Title");
        assert_eq!(project.author, "Jane Doe");

        // Verify book.toml was persisted
        let reloaded = BookProject::open(&temp_dir).expect("reopen");
        assert_eq!(reloaded.title, "Renamed Epic Title");
        assert_eq!(reloaded.author, "Jane Doe");

        let _ = fs::remove_dir_all(&temp_dir);
    }
    #[test]
    fn test_reorder_chapters_and_scenes() {
        let temp_dir = std::env::temp_dir().join("test_reorder_ch_sc");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut project =
            BookProject::create_novel_blueprint(&temp_dir, "Reorder Test", "Author")
                .expect("create blueprint");

        // Add 2 more chapters: ch02, ch03
        project.create_next_chapter().unwrap();
        project.create_next_chapter().unwrap();
        assert_eq!(project.chapters.len(), 3);

        // Add a second scene to chapter 0
        project.create_next_scene(0).unwrap();
        assert_eq!(project.chapters[0].scenes.len(), 2);

        // Test chapter reordering: move chapter 0 to index 2
        let ch0_name = project.chapters[0].name.clone();
        project.reorder_chapter(0, 2).unwrap();
        assert_eq!(project.chapters[2].name, ch0_name);

        // Test scene reordering within same chapter
        let sc0_name = project.chapters[2].scenes[0].name.clone();
        project.reorder_scene(2, 0, 2, 1).unwrap();
        assert_eq!(project.chapters[2].scenes[1].name, sc0_name);

        // Test moving a scene to another chapter
        let moved_path = project.reorder_scene(2, 1, 0, 0).unwrap();
        assert!(moved_path.exists());
        assert_eq!(project.chapters[0].scenes.len(), 2);
        assert_eq!(project.chapters[2].scenes.len(), 1);

        // Reopen project from disk to ensure persistence
        let reopened = BookProject::open(&temp_dir).unwrap();
        assert_eq!(reopened.chapters.len(), 3);
        assert_eq!(reopened.chapters[2].name, ch0_name);
        assert_eq!(reopened.chapters[0].scenes.len(), 2);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}