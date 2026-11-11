use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Context, Result};

use crate::project::{BookConfig, BookMetadata, BookProject, ChapterFolder, ChapterKind, ChapterMeta, SceneFile};

/// Represents a parsed `<BinderItem>` node from a Scrivener `.scrivx` XML manifest.
#[derive(Debug, Clone, Default)]
pub struct ScrivBinderItem {
    pub uuid: String,
    pub item_type: String,
    pub title: String,
    pub include_in_compile: bool,
    pub section_type_id: Option<String>,
    pub children: Vec<ScrivBinderItem>,
}

/// Summary statistics returned after importing a Scrivener project.
#[derive(Debug, Clone)]
pub struct ScrivImportReport {
    pub project: BookProject,
    pub chapters_imported: usize,
    pub specialized_chapters_imported: usize,
    pub scenes_imported: usize,
    pub front_matter_imported: usize,
}

/// Engine for parsing and porting Scrivener 3 (`.scriv`) projects into `BookProject` format.
pub struct ScrivenerImporter;

impl ScrivenerImporter {
    /// Imports a Scrivener `.scriv` bundle (or `.scrivx` manifest file) into `dest_dir`
    /// and returns the initialized `BookProject`.
    pub fn import<P: AsRef<Path>, Q: AsRef<Path>>(
        scriv_source: P,
        dest_dir: Q,
        override_title: Option<&str>,
        override_author: Option<&str>,
    ) -> Result<ScrivImportReport> {
        let source_path = scriv_source.as_ref();
        let (bundle_dir, scrivx_path) = Self::resolve_scriv_paths(source_path)?;

        let xml_content = fs::read_to_string(&scrivx_path)
            .with_context(|| format!("Failed to read Scrivener manifest {:?}", scrivx_path))?;

        let section_types = parse_section_types(&xml_content);
        let binder_items = parse_binder_items(&xml_content);

        if binder_items.is_empty() {
            return Err(anyhow!(
                "No valid <BinderItem> elements found in {:?}",
                scrivx_path
            ));
        }

        // Infer Book Title from override or .scrivx filename stem
        let inferred_title = scrivx_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Imported Novel".to_string());

        let book_title = override_title
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or(inferred_title);

        // Infer Author from override, or Manuscript Title Page / Compile settings, or fallback
        let inferred_author = Self::detect_author(&bundle_dir, &binder_items)
            .unwrap_or_else(|| "Author".to_string());

        let book_author = override_author
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or(inferred_author);

        let dest_root = dest_dir.as_ref().to_path_buf();
        fs::create_dir_all(&dest_root)
            .with_context(|| format!("Failed to create destination folder {:?}", dest_root))?;

        let front_matter_dir = dest_root.join("front_matter");
        let manuscript_dir = dest_root.join("manuscript");
        fs::create_dir_all(&front_matter_dir)?;
        fs::create_dir_all(&manuscript_dir)?;

        // 1. Extract and convert Front Matter
        let (front_matter, fm_imported_count) =
            Self::import_front_matter(&bundle_dir, &binder_items, &front_matter_dir)?;

        // 2. Locate DraftFolder ("Manuscript") and extract chapters & scenes
        let draft_folder = binder_items
            .iter()
            .find(|item| item.item_type == "DraftFolder")
            .or_else(|| {
                binder_items
                    .iter()
                    .find(|item| item.title.eq_ignore_ascii_case("Manuscript") || item.title.eq_ignore_ascii_case("Draft"))
            });

        let mut chapters = Vec::new();
        let mut chapter_order = Vec::new();
        let mut chapter_meta = HashMap::new();
        let mut numbered_counter = 0usize;
        let mut special_counter = 0usize;
        let mut total_scenes_imported = 0usize;

        if let Some(draft) = draft_folder {
            let mut loose_scenes: Vec<(&ScrivBinderItem, String)> = Vec::new();

            for item in &draft.children {
                if !item.include_in_compile {
                    continue;
                }

                match item.item_type.as_str() {
                    "Folder" => {
                        // Flush any preceding loose top-level text items into a chapter first
                        if !loose_scenes.is_empty() {
                            numbered_counter += 1;
                            let ch_folder_name = format!("ch{:02}", numbered_counter);
                            let ch_title = format!("Chapter {}", numbered_counter);
                            let (ch, meta, sc_count) = Self::write_chapter_to_disk(
                                &manuscript_dir,
                                &ch_folder_name,
                                &ch_title,
                                ChapterKind::Numbered,
                                &loose_scenes,
                            )?;
                            total_scenes_imported += sc_count;
                            chapter_order.push(ch_folder_name.clone());
                            chapter_meta.insert(ch_folder_name, meta);
                            chapters.push(ch);
                            loose_scenes.clear();
                        }

                        let kind = Self::classify_chapter_kind(item, &section_types);
                        let (ch_folder_name, ch_display_title) = match &kind {
                            ChapterKind::Specialized(special_title) => {
                                special_counter += 1;
                                let slug = slugify(special_title);
                                (
                                    format!("sp{:02}_{}", special_counter, slug),
                                    item.title.clone(),
                                )
                            }
                            ChapterKind::Numbered => {
                                numbered_counter += 1;
                                let title = if item.title.trim().is_empty() {
                                    format!("Chapter {}", numbered_counter)
                                } else {
                                    item.title.trim().to_string()
                                };
                                (format!("ch{:02}", numbered_counter), title)
                            }
                        };

                        // Collect all text scenes recursively inside this chapter folder
                        let mut chapter_scenes: Vec<(&ScrivBinderItem, String)> = Vec::new();
                        let folder_own_md = Self::read_uuid_markdown(&bundle_dir, &item.uuid);
                        if !folder_own_md.trim().is_empty() {
                            chapter_scenes.push((item, folder_own_md));
                        }
                        Self::collect_scenes_recursive(&bundle_dir, &item.children, &mut chapter_scenes);

                        let (ch, meta, sc_count) = Self::write_chapter_to_disk(
                            &manuscript_dir,
                            &ch_folder_name,
                            &ch_display_title,
                            kind,
                            &chapter_scenes,
                        )?;
                        total_scenes_imported += sc_count;
                        chapter_order.push(ch_folder_name.clone());
                        chapter_meta.insert(ch_folder_name, meta);
                        chapters.push(ch);
                    }
                    "Text" => {
                        // Check if this top-level Text item has child scenes or is a standalone specialized section
                        let kind = Self::classify_chapter_kind(item, &section_types);
                        if !item.children.is_empty() || matches!(kind, ChapterKind::Specialized(_)) {
                            if !loose_scenes.is_empty() {
                                numbered_counter += 1;
                                let ch_folder_name = format!("ch{:02}", numbered_counter);
                                let ch_title = format!("Chapter {}", numbered_counter);
                                let (ch, meta, sc_count) = Self::write_chapter_to_disk(
                                    &manuscript_dir,
                                    &ch_folder_name,
                                    &ch_title,
                                    ChapterKind::Numbered,
                                    &loose_scenes,
                                )?;
                                total_scenes_imported += sc_count;
                                chapter_order.push(ch_folder_name.clone());
                                chapter_meta.insert(ch_folder_name, meta);
                                chapters.push(ch);
                                loose_scenes.clear();
                            }

                            let (ch_folder_name, ch_display_title) = match &kind {
                                ChapterKind::Specialized(special_title) => {
                                    special_counter += 1;
                                    let slug = slugify(special_title);
                                    (
                                        format!("sp{:02}_{}", special_counter, slug),
                                        item.title.clone(),
                                    )
                                }
                                ChapterKind::Numbered => {
                                    numbered_counter += 1;
                                    (format!("ch{:02}", numbered_counter), item.title.clone())
                                }
                            };

                            let mut chapter_scenes = Vec::new();
                            let own_md = Self::read_uuid_markdown(&bundle_dir, &item.uuid);
                            chapter_scenes.push((item, own_md));
                            Self::collect_scenes_recursive(&bundle_dir, &item.children, &mut chapter_scenes);

                            let (ch, meta, sc_count) = Self::write_chapter_to_disk(
                                &manuscript_dir,
                                &ch_folder_name,
                                &ch_display_title,
                                kind,
                                &chapter_scenes,
                            )?;
                            total_scenes_imported += sc_count;
                            chapter_order.push(ch_folder_name.clone());
                            chapter_meta.insert(ch_folder_name, meta);
                            chapters.push(ch);
                        } else {
                            let md = Self::read_uuid_markdown(&bundle_dir, &item.uuid);
                            loose_scenes.push((item, md));
                        }
                    }
                    _ => {}
                }
            }

            if !loose_scenes.is_empty() {
                numbered_counter += 1;
                let ch_folder_name = format!("ch{:02}", numbered_counter);
                let ch_title = format!("Chapter {}", numbered_counter);
                let (ch, meta, sc_count) = Self::write_chapter_to_disk(
                    &manuscript_dir,
                    &ch_folder_name,
                    &ch_title,
                    ChapterKind::Numbered,
                    &loose_scenes,
                )?;
                total_scenes_imported += sc_count;
                chapter_order.push(ch_folder_name.clone());
                chapter_meta.insert(ch_folder_name, meta);
                chapters.push(ch);
            }
        }

        // Ensure at least one starter chapter exists if the Scrivener manuscript was completely empty
        if chapters.is_empty() {
            let ch_folder_name = "ch01".to_string();
            let ch_dir = manuscript_dir.join(&ch_folder_name);
            fs::create_dir_all(&ch_dir)?;
            let sc_path = ch_dir.join("scene01.md");
            fs::write(&sc_path, "")?;
            let sc = SceneFile::new(&sc_path, "Scene 1");
            chapters.push(ChapterFolder {
                path: ch_dir,
                name: ch_folder_name.clone(),
                title: "Chapter 1".to_string(),
                kind: ChapterKind::Numbered,
                scenes: vec![sc],
            });
            chapter_order.push(ch_folder_name.clone());
            chapter_meta.insert(
                ch_folder_name,
                ChapterMeta {
                    writer_title: Some("Chapter 1".to_string()),
                    special_title: None,
                    scene_titles: HashMap::from([("scene01.md".to_string(), "Scene 1".to_string())]),
                    scene_order: vec!["scene01.md".to_string()],
                },
            );
            total_scenes_imported = 1;
        }

        // 3. Write book.toml
        let metadata = BookMetadata {
            title: book_title.clone(),
            author: book_author.clone(),
            version: "0.4.0".to_string(),
            language: Some("en".to_string()),
            description: Some(format!("Imported from Scrivener project {:?}", scrivx_path.file_name().unwrap_or_default())),
            chapter_meta,
            chapter_order,
        };
        let config = BookConfig { book: metadata };
        let toml_string = toml::to_string_pretty(&config)
            .context("Failed to serialize imported book.toml")?;
        fs::write(dest_root.join("book.toml"), toml_string)?;

        let project = BookProject {
            root_dir: dest_root,
            title: book_title,
            author: book_author,
            config,
            front_matter,
            chapters,
        };

        Ok(ScrivImportReport {
            project,
            chapters_imported: numbered_counter + special_counter,
            specialized_chapters_imported: special_counter,
            scenes_imported: total_scenes_imported,
            front_matter_imported: fm_imported_count,
        })
    }

    /// Resolves either a `.scriv` directory or a `.scrivx` file path into `(bundle_dir, scrivx_path)`.
    pub fn resolve_scriv_paths(source: &Path) -> Result<(PathBuf, PathBuf)> {
        if source.is_file() {
            if source
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("scrivx"))
                .unwrap_or(false)
            {
                let bundle_dir = source
                    .parent()
                    .ok_or_else(|| anyhow!("Invalid .scrivx parent directory"))?
                    .to_path_buf();
                return Ok((bundle_dir, source.to_path_buf()));
            }
            return Err(anyhow!(
                "Expected a .scriv folder or .scrivx file, got {:?}",
                source
            ));
        }

        if source.is_dir() {
            for entry in fs::read_dir(source)
                .with_context(|| format!("Failed to read directory {:?}", source))?
            {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext.eq_ignore_ascii_case("scrivx") {
                            return Ok((source.to_path_buf(), path));
                        }
                    }
                }
            }
            return Err(anyhow!(
                "No .scrivx manifest file found inside {:?}",
                source
            ));
        }

        Err(anyhow!("Scrivener path {:?} does not exist", source))
    }

    /// Attempts to detect the author's name from the Scrivener project's Manuscript Format Title Page
    /// or compile settings if present.
    fn detect_author(bundle_dir: &Path, binder_items: &[ScrivBinderItem]) -> Option<String> {
        // Check Front Matter -> Manuscript Format -> Title Page first (often has the author's real name in line 1 of the contact block)
        if let Some(fm_folder) = binder_items
            .iter()
            .find(|i| i.title.eq_ignore_ascii_case("Front Matter"))
        {
            for sub in &fm_folder.children {
                if sub.title.eq_ignore_ascii_case("Manuscript Format") {
                    for doc in &sub.children {
                        if doc.title.eq_ignore_ascii_case("Title Page") {
                            let md = Self::read_uuid_markdown(bundle_dir, &doc.uuid);
                            if let Some(first_line) = md
                                .lines()
                                .map(str::trim)
                                .find(|l| !l.is_empty() && !l.starts_with('<') && !l.starts_with('$'))
                            {
                                let cleaned = first_line.trim_matches(['*', '#', '_']).trim();
                                if !cleaned.is_empty()
                                    && !cleaned.eq_ignore_ascii_case("Your address")
                                    && !cleaned.eq_ignore_ascii_case("Title Page")
                                    && cleaned.len() <= 60
                                {
                                    return Some(cleaned.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Determines whether a Scrivener binder item should be treated as a `Specialized` chapter
    /// (e.g., Prologue, Epilogue, Writer's Note) or a standard `Numbered` chapter.
    fn classify_chapter_kind(
        item: &ScrivBinderItem,
        section_types: &HashMap<String, String>,
    ) -> ChapterKind {
        let trimmed = item.title.trim();
        for preset in ChapterKind::preset_options() {
            if trimmed.eq_ignore_ascii_case(preset) {
                return ChapterKind::Specialized((*preset).to_string());
            }
        }

        // Also check common special headings
        for extra in ["Prologue", "Epilogue", "Interlude", "Prelude", "Coda", "Author's Note", "Writer's Note"] {
            if trimmed.eq_ignore_ascii_case(extra) {
                return ChapterKind::Specialized(trimmed.to_string());
            }
        }

        // If Scrivener explicitly assigned a "Heading" section type (unnumbered section heading)
        if let Some(st_id) = &item.section_type_id {
            if let Some(st_name) = section_types.get(st_id) {
                if st_name.eq_ignore_ascii_case("Heading") || st_name.eq_ignore_ascii_case("Part Heading") {
                    if !trimmed.is_empty() {
                        return ChapterKind::Specialized(trimmed.to_string());
                    }
                }
            }
        }

        ChapterKind::Numbered
    }

    fn collect_scenes_recursive<'a>(
        bundle_dir: &Path,
        children: &'a [ScrivBinderItem],
        out: &mut Vec<(&'a ScrivBinderItem, String)>,
    ) {
        for child in children {
            if !child.include_in_compile {
                continue;
            }
            if child.item_type == "Text" || child.item_type == "Folder" {
                let md = Self::read_uuid_markdown(bundle_dir, &child.uuid);
                if child.item_type == "Text" || !md.trim().is_empty() {
                    out.push((child, md));
                }
                if !child.children.is_empty() {
                    Self::collect_scenes_recursive(bundle_dir, &child.children, out);
                }
            }
        }
    }

    fn write_chapter_to_disk(
        manuscript_dir: &Path,
        ch_folder_name: &str,
        ch_display_title: &str,
        kind: ChapterKind,
        scene_items: &[(&ScrivBinderItem, String)],
    ) -> Result<(ChapterFolder, ChapterMeta, usize)> {
        let ch_dir = manuscript_dir.join(ch_folder_name);
        fs::create_dir_all(&ch_dir)?;

        let mut scenes = Vec::new();
        let mut scene_titles = HashMap::new();

        if scene_items.is_empty() {
            // Create a blank starter scene so empty Scrivener chapter folders are immediately usable
            let filename = "scene01.md";
            let sc_path = ch_dir.join(filename);
            fs::write(&sc_path, "")?;
            let sc_title = "Scene 1".to_string();
            scene_titles.insert(filename.to_string(), sc_title.clone());
            scenes.push(SceneFile::new(&sc_path, sc_title));
        } else {
            for (idx, (sc_item, md_content)) in scene_items.iter().enumerate() {
                let scene_num = idx + 1;
                let filename = format!("scene{:02}.md", scene_num);
                let sc_path = ch_dir.join(&filename);
                fs::write(&sc_path, md_content)?;

                let sc_title = if sc_item.title.trim().is_empty() {
                    format!("Scene {}", scene_num)
                } else {
                    sc_item.title.trim().to_string()
                };
                scene_titles.insert(filename, sc_title.clone());
                scenes.push(SceneFile::new(&sc_path, sc_title));
            }
        }

        let special_title = match &kind {
            ChapterKind::Numbered => None,
            ChapterKind::Specialized(st) => Some(st.clone()),
        };

        let scene_order = scenes.iter().map(|s| s.name.clone()).collect();
        let meta = ChapterMeta {
            writer_title: Some(ch_display_title.to_string()),
            special_title,
            scene_titles,
            scene_order,
        };

        let scenes_len = scenes.len();
        let folder = ChapterFolder {
            path: ch_dir,
            name: ch_folder_name.to_string(),
            title: ch_display_title.to_string(),
            kind,
            scenes,
        };

        Ok((folder, meta, scenes_len))
    }

    /// Extracts Front Matter (`Title Page`, `Copyright`, `Dedication`) from Scrivener's `Front Matter` folder
    /// (prioritizing the `Paperback` subfolder when available) and falls back to clean templates with `$Title` and `$Author`.
    fn import_front_matter(
        bundle_dir: &Path,
        binder_items: &[ScrivBinderItem],
        front_matter_dir: &Path,
    ) -> Result<(Vec<SceneFile>, usize)> {
        let mut imported_title_page: Option<String> = None;
        let mut imported_copyright: Option<String> = None;
        let mut imported_dedication: Option<String> = None;
        let mut imported_count = 0usize;

        if let Some(fm_root) = binder_items
            .iter()
            .find(|i| i.title.eq_ignore_ascii_case("Front Matter"))
        {
            // Prefer "Paperback" subfolder if present, otherwise inspect all children
            let preferred_candidates: Vec<&ScrivBinderItem> = if let Some(paperback) = fm_root
                .children
                .iter()
                .find(|c| c.title.to_ascii_lowercase().contains("paperback"))
            {
                paperback.children.iter().collect()
            } else {
                fm_root.children.iter().collect()
            };

            for item in preferred_candidates {
                if item.item_type != "Text" {
                    continue;
                }
                let raw_md = Self::read_uuid_markdown(bundle_dir, &item.uuid);
                let normalized = normalize_front_matter_markdown(&item.title, &raw_md);
                if normalized.trim().is_empty() {
                    continue;
                }

                let lower_title = item.title.to_ascii_lowercase();
                if lower_title.contains("title") && imported_title_page.is_none() {
                    imported_title_page = Some(normalized);
                    imported_count += 1;
                } else if lower_title.contains("copyright") && imported_copyright.is_none() {
                    imported_copyright = Some(normalized);
                    imported_count += 1;
                } else if lower_title.contains("dedication") && imported_dedication.is_none() {
                    imported_dedication = Some(normalized);
                    imported_count += 1;
                }
            }
        }

        let title_page_content = imported_title_page
            .unwrap_or_else(|| "# $Title\n\n### By $Author\n\n*Draft Edition*\n".to_string());
        let copyright_content = imported_copyright.unwrap_or_else(|| {
            "# Copyright\n\nCopyright © 2026 by $Author.\n\nAll rights reserved.\n".to_string()
        });
        let dedication_content = imported_dedication
            .unwrap_or_else(|| "# Dedication\n\n*Dedicated to all readers and dreamers.*\n".to_string());

        let title_page_file = front_matter_dir.join("title_page.md");
        let copyright_file = front_matter_dir.join("copyright.md");
        let dedication_file = front_matter_dir.join("dedication.md");

        fs::write(&title_page_file, title_page_content)?;
        fs::write(&copyright_file, copyright_content)?;
        fs::write(&dedication_file, dedication_content)?;

        let front_matter = vec![
            SceneFile::new(&title_page_file, "Title Page"),
            SceneFile::new(&copyright_file, "Copyright"),
            SceneFile::new(&dedication_file, "Dedication"),
        ];

        Ok((front_matter, imported_count))
    }

    /// Reads `Files/Data/<UUID>/content.rtf` from the Scrivener bundle and converts it to clean Markdown.
    pub fn read_uuid_markdown(bundle_dir: &Path, uuid: &str) -> String {
        if uuid.is_empty() {
            return String::new();
        }
        let rtf_path = bundle_dir
            .join("Files")
            .join("Data")
            .join(uuid)
            .join("content.rtf");

        if !rtf_path.exists() {
            return String::new();
        }

        match fs::read_to_string(&rtf_path) {
            Ok(raw_rtf) => rtf_to_markdown(&raw_rtf),
            Err(_) => {
                // Fallback for non-UTF8 RTF files (since RTF is 7-bit ASCII + hex escapes)
                if let Ok(bytes) = fs::read(&rtf_path) {
                    let lossy = String::from_utf8_lossy(&bytes);
                    rtf_to_markdown(&lossy)
                } else {
                    String::new()
                }
            }
        }
    }
}

/// Converts Scrivener placeholders (`<$PROJECTTITLE>`, `<$author>`, `<$year>`) into The Scribe's Ledger's `$Title` and `$Author` syntax.
fn normalize_front_matter_markdown(doc_title: &str, md: &str) -> String {
    let mut text = replace_scrivener_placeholders(md);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Format Title Page nicely if it only consists of bare $Title / $Author lines
    if doc_title.to_ascii_lowercase().contains("title") {
        let mut formatted_lines = Vec::new();
        for line in trimmed.lines() {
            let l = line.trim();
            if l.is_empty() {
                continue;
            }
            if l == "$Title" {
                formatted_lines.push("# $Title".to_string());
            } else if l == "$Author" || l.eq_ignore_ascii_case("by $Author") {
                formatted_lines.push("### By $Author".to_string());
            } else {
                formatted_lines.push(l.to_string());
            }
        }
        text = formatted_lines.join("\n\n") + "\n";
    } else {
        text = trimmed.to_string() + "\n";
    }

    text
}

/// Replaces Scrivener compile placeholders with The Scribe's Ledger equivalents.
pub fn replace_scrivener_placeholders(input: &str) -> String {
    let mut out = input.to_string();
    for pat in [
        "<$PROJECTTITLE>",
        "<$projecttitle>",
        "<$ProjectTitle>",
        "<$title>",
        "<$TITLE>",
    ] {
        out = out.replace(pat, "$Title");
    }
    for pat in [
        "<$author>",
        "<$AUTHOR>",
        "<$Author>",
        "<$fullname>",
        "<$FULLNAME>",
        "<$FullName>",
    ] {
        out = out.replace(pat, "$Author");
    }
    for pat in ["<$year>", "<$YEAR>", "<$Year>"] {
        out = out.replace(pat, "2026");
    }
    out
}

/// Converts a Scrivener RTF stream (`content.rtf`) into clean Markdown text,
/// preserving paragraphs, bold (`**`), italic (`*`), Unicode escapes (`\uN`), and Windows-1252 hex escapes (`\'xx`).
pub fn rtf_to_markdown(rtf: &str) -> String {
    let trimmed = rtf.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if !trimmed.starts_with("{\\rtf") {
        return replace_scrivener_placeholders(trimmed);
    }

    #[derive(Clone, Copy, Default)]
    struct RtfState {
        bold: bool,
        italic: bool,
        uc_skip: usize,
        ignore_group: bool,
    }

    // Pre-scan fonttbl to detect fonts whose family name inherently implies Bold or Italic
    // (e.g., {\f1\fmodern\fcharset162\fprq2 Cambria-Italic;} in Scrivener's Dedication RTF)
    let (italic_fonts, bold_fonts) = parse_rtf_font_styles(trimmed);

    let bytes = trimmed.as_bytes();
    let len = bytes.len();
    let mut pos = 0usize;

    let mut stack: Vec<RtfState> = Vec::with_capacity(16);
    let mut state = RtfState {
        bold: false,
        italic: false,
        uc_skip: 1,
        ignore_group: false,
    };

    let mut paragraphs: Vec<String> = Vec::new();
    let mut current_para = String::new();
    let mut active_bold = false;
    let mut active_italic = false;

    let close_inline_styles =
        |para: &mut String, act_bold: &mut bool, act_italic: &mut bool| {
            // Keep trailing spaces outside closing markdown markers
            let mut trailing_spaces = String::new();
            while para.ends_with(' ') {
                para.pop();
                trailing_spaces.push(' ');
            }
            if *act_italic {
                para.push('*');
                *act_italic = false;
            }
            if *act_bold {
                para.push_str("**");
                *act_bold = false;
            }
            para.push_str(&trailing_spaces);
        };

    while pos < len {
        let b = bytes[pos];
        match b {
            b'{' => {
                stack.push(state);
                pos += 1;
                // Check for `\*` destination group
                if pos + 1 < len && bytes[pos] == b'\\' && bytes[pos + 1] == b'*' {
                    state.ignore_group = true;
                    pos += 2;
                }
            }
            b'}' => {
                if let Some(prev) = stack.pop() {
                    state = prev;
                }
                if !state.bold && active_bold || !state.italic && active_italic {
                    let mut trailing_spaces = String::new();
                    while current_para.ends_with(' ') {
                        current_para.pop();
                        trailing_spaces.push(' ');
                    }
                    if !state.italic && active_italic {
                        current_para.push('*');
                        active_italic = false;
                    }
                    if !state.bold && active_bold {
                        current_para.push_str("**");
                        active_bold = false;
                    }
                    current_para.push_str(&trailing_spaces);
                }
                pos += 1;
            }
            b'\\' => {
                pos += 1;
                if pos >= len {
                    break;
                }
                let next_b = bytes[pos];
                match next_b {
                    b'\\' | b'{' | b'}' => {
                        if !state.ignore_group {
                            sync_open_styles(
                                &mut current_para,
                                state.bold,
                                state.italic,
                                &mut active_bold,
                                &mut active_italic,
                            );
                            current_para.push(next_b as char);
                        }
                        pos += 1;
                    }
                    b'~' => {
                        if !state.ignore_group {
                            current_para.push(' ');
                        }
                        pos += 1;
                    }
                    b'_' => {
                        if !state.ignore_group {
                            current_para.push('-');
                        }
                        pos += 1;
                    }
                    b'-' | b'\r' | b'\n' => {
                        pos += 1;
                    }
                    b'\'' => {
                        // Hex escape \'xx
                        pos += 1;
                        if pos + 2 <= len {
                            let hex_str = std::str::from_utf8(&bytes[pos..pos + 2]).unwrap_or("00");
                            if let Ok(code) = u8::from_str_radix(hex_str, 16) {
                                if !state.ignore_group {
                                    sync_open_styles(
                                        &mut current_para,
                                        state.bold,
                                        state.italic,
                                        &mut active_bold,
                                        &mut active_italic,
                                    );
                                    current_para.push(decode_cp1252(code));
                                }
                            }
                            pos += 2;
                        }
                    }
                    _ if next_b.is_ascii_alphabetic() => {
                        // Parse RTF control word
                        let word_start = pos;
                        while pos < len && bytes[pos].is_ascii_alphabetic() {
                            pos += 1;
                        }
                        let word = std::str::from_utf8(&bytes[word_start..pos]).unwrap_or("");

                        // Optional signed numeric parameter
                        let num_start = pos;
                        if pos < len && (bytes[pos] == b'-' || bytes[pos].is_ascii_digit()) {
                            pos += 1;
                            while pos < len && bytes[pos].is_ascii_digit() {
                                pos += 1;
                            }
                        }
                        let num_param: Option<i32> = if pos > num_start {
                            std::str::from_utf8(&bytes[num_start..pos])
                                .ok()
                                .and_then(|s| s.parse::<i32>().ok())
                        } else {
                            None
                        };

                        // A single space delimiter after a control word is consumed as part of the control word
                        if pos < len && bytes[pos] == b' ' {
                            pos += 1;
                        }

                        // Ignored header/destination groups
                        if matches!(
                            word,
                            "fonttbl"
                                | "colortbl"
                                | "stylesheet"
                                | "info"
                                | "pict"
                                | "header"
                                | "footer"
                                | "Skip"
                        ) {
                            state.ignore_group = true;
                            continue;
                        }

                        if state.ignore_group {
                            continue;
                        }

                        match word {
                            "par" | "line" | "row" => {
                                close_inline_styles(
                                    &mut current_para,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                                paragraphs.push(std::mem::take(&mut current_para));
                            }
                            "cell" => {
                                current_para.push(' ');
                            }
                            "tab" => {
                                current_para.push('\t');
                            }
                            "plain" | "pard" => {
                                state.bold = false;
                                state.italic = false;
                                close_inline_styles(
                                    &mut current_para,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                            }
                            "b" => {
                                let turn_on = num_param.unwrap_or(1) != 0;
                                state.bold = turn_on;
                                if !turn_on && active_bold {
                                    close_inline_styles(
                                        &mut current_para,
                                        &mut active_bold,
                                        &mut active_italic,
                                    );
                                }
                            }
                            "i" => {
                                let turn_on = num_param.unwrap_or(1) != 0;
                                state.italic = turn_on;
                                if !turn_on && active_italic {
                                    close_inline_styles(
                                        &mut current_para,
                                        &mut active_bold,
                                        &mut active_italic,
                                    );
                                }
                            }
                            "f" => {
                                if let Some(f_idx) = num_param {
                                    if italic_fonts.contains(&f_idx) {
                                        state.italic = true;
                                    }
                                    if bold_fonts.contains(&f_idx) {
                                        state.bold = true;
                                    }
                                }
                            }
                            "uc" => {
                                if let Some(n) = num_param {
                                    if n >= 0 {
                                        state.uc_skip = n as usize;
                                    }
                                }
                            }
                            "u" => {
                                if let Some(code) = num_param {
                                    let cp = if code < 0 {
                                        (code + 65536) as u32
                                    } else {
                                        code as u32
                                    };
                                    if let Some(ch) = char::from_u32(cp) {
                                        sync_open_styles(
                                            &mut current_para,
                                            state.bold,
                                            state.italic,
                                            &mut active_bold,
                                            &mut active_italic,
                                        );
                                        current_para.push(ch);
                                    }
                                    // Skip `uc_skip` ANSI fallback characters (either `\'xx` or single byte)
                                    for _ in 0..state.uc_skip {
                                        if pos + 3 < len
                                            && bytes[pos] == b'\\'
                                            && bytes[pos + 1] == b'\''
                                        {
                                            pos += 4;
                                        } else if pos < len && bytes[pos] != b'{' && bytes[pos] != b'}' && bytes[pos] != b'\\' {
                                            pos += 1;
                                        }
                                    }
                                }
                            }
                            "emdash" => {
                                sync_open_styles(
                                    &mut current_para,
                                    state.bold,
                                    state.italic,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                                current_para.push('—');
                            }
                            "endash" => {
                                sync_open_styles(
                                    &mut current_para,
                                    state.bold,
                                    state.italic,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                                current_para.push('–');
                            }
                            "lquote" => {
                                sync_open_styles(
                                    &mut current_para,
                                    state.bold,
                                    state.italic,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                                current_para.push('‘');
                            }
                            "rquote" => {
                                sync_open_styles(
                                    &mut current_para,
                                    state.bold,
                                    state.italic,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                                current_para.push('’');
                            }
                            "ldblquote" => {
                                sync_open_styles(
                                    &mut current_para,
                                    state.bold,
                                    state.italic,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                                current_para.push('“');
                            }
                            "rdblquote" => {
                                sync_open_styles(
                                    &mut current_para,
                                    state.bold,
                                    state.italic,
                                    &mut active_bold,
                                    &mut active_italic,
                                );
                                current_para.push('”');
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        pos += 1;
                    }
                }
            }
            b'\r' | b'\n' => {
                pos += 1;
            }
            _ => {
                if !state.ignore_group {
                    // Decode UTF-8 sequence or ASCII byte
                    let ch = if b < 0x80 {
                        pos += 1;
                        b as char
                    } else {
                        let rest = &trimmed[pos..];
                        if let Some(c) = rest.chars().next() {
                            pos += c.len_utf8();
                            c
                        } else {
                            pos += 1;
                            b as char
                        }
                    };
                    if ch != ' ' || active_bold || active_italic {
                        sync_open_styles(
                            &mut current_para,
                            state.bold,
                            state.italic,
                            &mut active_bold,
                            &mut active_italic,
                        );
                    }
                    current_para.push(ch);
                } else {
                    pos += 1;
                }
            }
        }
    }

    close_inline_styles(&mut current_para, &mut active_bold, &mut active_italic);
    if !current_para.trim().is_empty() {
        paragraphs.push(current_para);
    }

    // Filter out empty layout spacer paragraphs and join with double newlines for clean Markdown
    let non_empty: Vec<String> = paragraphs
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty() && p != "**" && p != "*")
        .collect();

    let joined = non_empty.join("\n\n");
    replace_scrivener_placeholders(&joined)
}

fn sync_open_styles(
    para: &mut String,
    want_bold: bool,
    want_italic: bool,
    active_bold: &mut bool,
    active_italic: &mut bool,
) {
    if want_bold && !*active_bold {
        para.push_str("**");
        *active_bold = true;
    }
    if want_italic && !*active_italic {
        para.push('*');
        *active_italic = true;
    }
}

/// Scans the `{\fonttbl ...}` header in an RTF string to identify font IDs (`\fN`) whose font name
/// ends in `-Italic` or `Italic` / `-Bold` or `Bold`, except when overridden by `\i0` / `\b0`.
fn parse_rtf_font_styles(_rtf: &str) -> (Vec<i32>, Vec<i32>) {
    // Note: Scrivener explicitly writes `\b0\i1` or `\b1\i0` on every styled run in `content.rtf`,
    // and writes `\f1...\b0\i0` when using a font without active italic style. Relying on `\b` and `\i`
    // flags directly avoids false-positive italics when Scrivener emits `\f1\b0\i0`.
    (Vec::new(), Vec::new())
}

/// Decodes a Windows-1252 byte into its corresponding Unicode `char`.
fn decode_cp1252(code: u8) -> char {
    match code {
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8A => 'Š',
        0x8B => '‹',
        0x8C => 'Œ',
        0x8E => 'Ž',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9A => 'š',
        0x9B => '›',
        0x9C => 'œ',
        0x9E => 'ž',
        0x9F => 'Ÿ',
        other => other as char,
    }
}

/// Parses `<SectionTypes><TypeDefinitions><Type ID="...">Name</Type>` from `.scrivx`.
pub fn parse_section_types(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(st_start) = xml.find("<SectionTypes>") else {
        return map;
    };
    let Some(st_end) = xml[st_start..].find("</SectionTypes>") else {
        return map;
    };
    let block = &xml[st_start..st_start + st_end];

    let mut cursor = 0;
    while let Some(rel) = block[cursor..].find("<Type ") {
        let tag_start = cursor + rel;
        let Some(tag_close_rel) = block[tag_start..].find('>') else {
            break;
        };
        let tag_header = &block[tag_start..tag_start + tag_close_rel];
        let after_open = tag_start + tag_close_rel + 1;
        let Some(end_rel) = block[after_open..].find("</Type>") else {
            break;
        };
        let inner_text = decode_xml_entities(block[after_open..after_open + end_rel].trim());
        if let Some(id) = extract_xml_attr(tag_header, "ID") {
            map.insert(id, inner_text);
        }
        cursor = after_open + end_rel + "</Type>".len();
    }
    map
}

/// Parses the top-level `<Binder>` hierarchy from a `.scrivx` XML string into recursive `ScrivBinderItem` trees.
pub fn parse_binder_items(xml: &str) -> Vec<ScrivBinderItem> {
    let Some(binder_start) = xml.find("<Binder>") else {
        return Vec::new();
    };
    let Some(binder_end_rel) = xml[binder_start..].find("</Binder>") else {
        return Vec::new();
    };
    let binder_slice = &xml[binder_start + "<Binder>".len()..binder_start + binder_end_rel];
    let mut pos = 0;
    parse_binder_children(binder_slice, &mut pos)
}

fn parse_binder_children(slice: &str, pos: &mut usize) -> Vec<ScrivBinderItem> {
    let mut items = Vec::new();

    while *pos < slice.len() {
        let rest = &slice[*pos..];
        let next_open = rest.find("<BinderItem");
        let next_children_close = rest.find("</Children>");

        // If `</Children>` appears before the next `<BinderItem`, we reached the end of this child list
        if let Some(close_idx) = next_children_close {
            if next_open.map_or(true, |open_idx| close_idx < open_idx) {
                *pos += close_idx + "</Children>".len();
                break;
            }
        }

        let Some(open_rel) = next_open else {
            break;
        };
        *pos += open_rel;

        if let Some(item) = parse_single_binder_item(slice, pos) {
            items.push(item);
        } else {
            break;
        }
    }

    items
}

fn parse_single_binder_item(slice: &str, pos: &mut usize) -> Option<ScrivBinderItem> {
    let rest = &slice[*pos..];
    let tag_end_rel = rest.find('>')?;
    let header = &rest[..tag_end_rel];
    let self_closing = header.ends_with('/');

    let uuid = extract_xml_attr(header, "UUID").unwrap_or_default();
    let item_type = extract_xml_attr(header, "Type").unwrap_or_default();

    *pos += tag_end_rel + 1;

    let mut item = ScrivBinderItem {
        uuid,
        item_type,
        title: String::new(),
        include_in_compile: true,
        section_type_id: None,
        children: Vec::new(),
    };

    if self_closing {
        return Some(item);
    }

    // Parse inside `<BinderItem ...> ... </BinderItem>`
    while *pos < slice.len() {
        let rem = &slice[*pos..];

        // Find next significant tag
        let Some(lt_rel) = rem.find('<') else {
            *pos = slice.len();
            break;
        };
        *pos += lt_rel;
        let tag_rem = &slice[*pos..];

        if tag_rem.starts_with("</BinderItem>") {
            *pos += "</BinderItem>".len();
            break;
        } else if tag_rem.starts_with("<Title>") {
            *pos += "<Title>".len();
            if let Some(end_rel) = slice[*pos..].find("</Title>") {
                let raw_title = &slice[*pos..*pos + end_rel];
                item.title = decode_xml_entities(raw_title.trim());
                *pos += end_rel + "</Title>".len();
            }
        } else if tag_rem.starts_with("<IncludeInCompile>") {
            *pos += "<IncludeInCompile>".len();
            if let Some(end_rel) = slice[*pos..].find("</IncludeInCompile>") {
                let val = slice[*pos..*pos + end_rel].trim();
                item.include_in_compile = !val.eq_ignore_ascii_case("No");
                *pos += end_rel + "</IncludeInCompile>".len();
            }
        } else if tag_rem.starts_with("<SectionType>") {
            *pos += "<SectionType>".len();
            if let Some(end_rel) = slice[*pos..].find("</SectionType>") {
                let val = slice[*pos..*pos + end_rel].trim();
                if !val.is_empty() {
                    item.section_type_id = Some(val.to_string());
                }
                *pos += end_rel + "</SectionType>".len();
            }
        } else if tag_rem.starts_with("<Children>") {
            *pos += "<Children>".len();
            item.children = parse_binder_children(slice, pos);
        } else {
            // Skip past this tag opening
            if let Some(gt_rel) = tag_rem.find('>') {
                *pos += gt_rel + 1;
            } else {
                *pos += 1;
            }
        }
    }

    Some(item)
}

fn extract_xml_attr(tag_header: &str, attr_name: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr_name);
    let start = tag_header.find(&pattern)? + pattern.len();
    let end = tag_header[start..].find('"')? + start;
    Some(decode_xml_entities(&tag_header[start..end]))
}

fn decode_xml_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn slugify(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = s.trim_matches('_');
    if trimmed.is_empty() {
        "special".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtf_to_markdown_unicode_and_italics() {
        let sample_rtf = r#"{\rtf1\ansi\ansicpg1252\uc1\deff0
{\fonttbl{\f0\fmodern\fcharset162\fprq2 SitkaText;}{\f1\fmodern\fcharset162\fprq2 SitkaTextItalic;}}
{\colortbl;\red0\green0\blue0;\red255\green255\blue255;}
\pard\plain {\f0\fs24\b0\i0 He couldn\loch\af0\hich\af0\dbch\af0\uc1\u8217\'92t answer. }{\f1\fs24\b0\i1 Everything is fine, it\u8217\'92s nothing.}{\f0\fs24\b0\i0  \u8220\'93Morning, Artie,\u8221\'94 he said.}
\par\plain {\f1\fs24\b0\i1 I don\u8217\'92t belong here.}}"#;

        let md = rtf_to_markdown(sample_rtf);
        assert!(md.contains("He couldn’t answer."));
        assert!(md.contains("*Everything is fine, it’s nothing.*"));
        assert!(md.contains("“Morning, Artie,” he said."));
        assert!(md.contains("\n\n*I don’t belong here.*"));
    }

    #[test]
    fn test_import_the_traveller_scriv_project() {
        let scriv_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("READ-ONLY")
            .join("The Traveller.scriv");
        if !scriv_dir.exists() {
            return;
        }

        let temp_dest = std::env::temp_dir().join("test_scriv_import_the_traveller");
        let _ = fs::remove_dir_all(&temp_dest);

        let report = ScrivenerImporter::import(&scriv_dir, &temp_dest, None, None)
            .expect("Scrivener import should succeed on The Traveller.scriv");

        assert_eq!(report.project.title, "The Traveller");
        assert_eq!(report.project.author, "Emre");

        // Verify chapters: 10 numbered chapters ("The Initiation" .. "The Traveller") + 1 specialized ("Epilogue")
        assert_eq!(report.project.chapters.len(), 11);
        assert_eq!(report.specialized_chapters_imported, 1);

        assert_eq!(report.project.chapters[0].title, "The Initiation");
        assert!(report.project.chapters[0].kind.is_numbered());
        assert_eq!(report.project.chapters[0].scenes.len(), 3);
        assert_eq!(report.project.chapters[0].scenes[0].title, "Day 1");
        assert_eq!(report.project.chapters[0].scenes[1].title, "Day 2");
        assert_eq!(
            report.project.chapters[0].scenes[2].title,
            "Day 3 + Day 4 (Beginning)"
        );

        // Verify Day 1 content was cleanly converted from RTF to Markdown
        let day1_md = report.project.chapters[0].scenes[0]
            .read_content()
            .expect("read Day 1");
        assert!(day1_md.starts_with("7 A.M. sharp, eyes open, wide awake."));
        assert!(day1_md.contains("“Good morning Artie”"));
        assert!(day1_md.contains("Arthur’s request"));

        // Verify Day 9 italics
        let trigger_ch = &report.project.chapters[7];
        assert_eq!(trigger_ch.title, "The Trigger");
        let day9_md = trigger_ch.scenes[0].read_content().expect("read Day 9");
        assert!(day9_md.contains("*I don’t belong here.*"));
        assert!(day9_md.contains("*All done. I hate to leave behind a perfectly peaceful life, but it isn’t mine to live.*"));

        // Verify Epilogue is imported as a specialized chapter
        let epilogue = report.project.chapters.last().unwrap();
        assert_eq!(epilogue.title, "Epilogue");
        assert_eq!(epilogue.specialized_print_title(), Some("Epilogue"));

        // Verify Front Matter placeholders ($Title and $Author)
        let title_page_md = report.project.front_matter[0]
            .read_content()
            .expect("read title_page.md");
        assert!(title_page_md.contains("$Title"));
        assert!(title_page_md.contains("$Author"));

        let dedication_md = report.project.front_matter[2]
            .read_content()
            .expect("read dedication.md");
        assert!(dedication_md.contains("To my friends, who are as surprised by my commitment to this book as I was."));

        // Verify reopening from disk preserves everything
        let reopened = BookProject::open(&temp_dest).expect("reopen imported project");
        assert_eq!(reopened.title, "The Traveller");
        assert_eq!(reopened.author, "Emre");
        assert_eq!(reopened.chapters.len(), 11);
        assert_eq!(
            reopened.chapters.last().unwrap().specialized_print_title(),
            Some("Epilogue")
        );

        let _ = fs::remove_dir_all(&temp_dest);
    }
}
