use crate::app::{BookApp, SelectedDocument};
use eframe::egui;
use std::path::PathBuf;
use std::time::Instant;

/// Scope of a Find / Change Findings operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindScope {
    Scene,
    Chapter,
    Manuscript,
}

impl FindScope {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Scene => "Scene",
            Self::Chapter => "Chapter",
            Self::Manuscript => "Manuscript",
        }
    }
}

/// A single search finding with its document location and UTF-8 / char offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMatch {
    pub doc: SelectedDocument,
    pub doc_title: String,
    pub chapter_title: Option<String>,
    pub byte_start: usize,
    pub byte_end: usize,
    pub char_start: usize,
    pub char_end: usize,
}

/// Finds all non-overlapping occurrences of `needle` inside `haystack`.
/// Returns `(byte_start, byte_end, char_start, char_end)` tuples guaranteed to lie on UTF-8 char boundaries.
pub fn find_all_occurrences(
    haystack: &str,
    needle: &str,
    case_sensitive: bool,
) -> Vec<(usize, usize, usize, usize)> {
    if needle.is_empty() || haystack.is_empty() {
        return Vec::new();
    }

    if case_sensitive {
        let needle_chars = needle.chars().count();
        let mut results = Vec::new();
        let mut prev_byte = 0usize;
        let mut prev_char = 0usize;
        for (b_start, matched) in haystack.match_indices(needle) {
            let delta_chars = haystack[prev_byte..b_start].chars().count();
            let c_start = prev_char + delta_chars;
            let c_end = c_start + needle_chars;
            let b_end = b_start + matched.len();
            results.push((b_start, b_end, c_start, c_end));
            prev_byte = b_end;
            prev_char = c_end;
        }
        return results;
    }

    let needle_lower: Vec<char> = needle.chars().flat_map(|c| c.to_lowercase()).collect();
    if needle_lower.is_empty() {
        return Vec::new();
    }

    let mut hay_lower: Vec<(usize, usize, usize, char)> = Vec::with_capacity(haystack.len());
    for (c_idx, (b_start, ch)) in haystack.char_indices().enumerate() {
        let b_end = b_start + ch.len_utf8();
        for lc in ch.to_lowercase() {
            hay_lower.push((b_start, b_end, c_idx, lc));
        }
    }

    let n_len = needle_lower.len();
    if hay_lower.len() < n_len {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut i = 0usize;
    while i + n_len <= hay_lower.len() {
        let starts_on_char = i == 0 || hay_lower[i].2 != hay_lower[i - 1].2;
        let end_idx = i + n_len;
        let ends_on_char =
            end_idx == hay_lower.len() || hay_lower[end_idx].2 != hay_lower[end_idx - 1].2;

        if starts_on_char
            && ends_on_char
            && (0..n_len).all(|k| hay_lower[i + k].3 == needle_lower[k])
        {
            let b_start = hay_lower[i].0;
            let b_end = hay_lower[end_idx - 1].1;
            let c_start = hay_lower[i].2;
            let c_end = hay_lower[end_idx - 1].2 + 1;
            results.push((b_start, b_end, c_start, c_end));
            i = end_idx;
            continue;
        }
        i += 1;
    }

    results
}

/// Replaces all non-overlapping occurrences of `needle` with `replacement` in `text`.
/// Returns `(new_text, replacement_count)`.
pub fn replace_all_in_text(
    text: &str,
    needle: &str,
    replacement: &str,
    case_sensitive: bool,
) -> (String, usize) {
    let matches = find_all_occurrences(text, needle, case_sensitive);
    if matches.is_empty() {
        return (text.to_string(), 0);
    }

    let count = matches.len();
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for &(b_start, b_end, _, _) in &matches {
        out.push_str(&text[last..b_start]);
        out.push_str(replacement);
        last = b_end;
    }
    out.push_str(&text[last..]);
    (out, count)
}

/// Overlays search highlight backgrounds onto an already-constructed `LayoutJob` without
/// altering `job.text` or markdown formatting spans.
pub fn apply_search_highlights_to_job(
    job: &mut egui::text::LayoutJob,
    query: &str,
    case_sensitive: bool,
    current_match_byte_start: Option<usize>,
    is_dark_paper: bool,
) {
    if query.is_empty() || job.text.is_empty() {
        return;
    }

    let occurrences = find_all_occurrences(&job.text, query, case_sensitive);
    if occurrences.is_empty() {
        return;
    }

    let current_bg = egui::Color32::from_rgb(245, 166, 35);
    let current_fg = egui::Color32::from_rgb(20, 24, 20);
    let other_bg = if is_dark_paper {
        egui::Color32::from_rgba_unmultiplied(230, 195, 70, 95)
    } else {
        egui::Color32::from_rgba_unmultiplied(255, 228, 105, 165)
    };

    let old_sections = std::mem::take(&mut job.sections);
    let mut new_sections = Vec::with_capacity(old_sections.len() + occurrences.len() * 2);

    let mut occ_idx = 0usize;
    for sec in old_sections {
        if sec.format.color == egui::Color32::TRANSPARENT || sec.byte_range.is_empty() {
            new_sections.push(sec);
            continue;
        }

        let sec_start = sec.byte_range.start;
        let sec_end = sec.byte_range.end;

        while occ_idx < occurrences.len() && occurrences[occ_idx].1 <= sec_start {
            occ_idx += 1;
        }

        let mut cursor = sec_start;
        let mut is_first_piece = true;
        let mut scan = occ_idx;

        while scan < occurrences.len() {
            let (m_start, m_end, _, _) = occurrences[scan];
            if m_start >= sec_end {
                break;
            }

            let overlap_start = m_start.max(cursor);
            let overlap_end = m_end.min(sec_end);

            if overlap_start > cursor {
                new_sections.push(egui::text::LayoutSection {
                    leading_space: if is_first_piece {
                        sec.leading_space
                    } else {
                        0.0
                    },
                    byte_range: cursor..overlap_start,
                    format: sec.format.clone(),
                });
                is_first_piece = false;
            }

            if overlap_end > overlap_start {
                let mut fmt = sec.format.clone();
                if current_match_byte_start == Some(m_start) {
                    fmt.background = current_bg;
                    fmt.color = current_fg;
                } else {
                    fmt.background = other_bg;
                }
                new_sections.push(egui::text::LayoutSection {
                    leading_space: if is_first_piece {
                        sec.leading_space
                    } else {
                        0.0
                    },
                    byte_range: overlap_start..overlap_end,
                    format: fmt,
                });
                is_first_piece = false;
                cursor = overlap_end;
            }

            if m_end <= sec_end {
                scan += 1;
            } else {
                break;
            }
        }

        if cursor < sec_end {
            new_sections.push(egui::text::LayoutSection {
                leading_space: if is_first_piece {
                    sec.leading_space
                } else {
                    0.0
                },
                byte_range: cursor..sec_end,
                format: sec.format,
            });
        }
    }

    job.sections = new_sections;
}

impl BookApp {
    /// Opens the Find pop-up on the top-left of the page panel and focuses its search input.
    pub fn open_find_popup(&mut self) {
        self.show_find_popup = true;
        self.find_focus_requested = true;
        self.refresh_find_matches();
        if !self.find_matches.is_empty() {
            self.find_scroll_to_match = true;
        }
    }

    /// Recomputes all findings for the current query and selected scope (`Scene`, `Chapter`, `Manuscript`).
    pub fn refresh_find_matches(&mut self) {
        let prev_key = self
            .find_matches
            .get(self.find_current_idx)
            .map(|m| (m.doc.clone(), m.byte_start));

        self.find_matches.clear();

        if self.find_query.is_empty() {
            self.find_current_idx = 0;
            return;
        }

        let query = self.find_query.clone();
        let case_sensitive = self.find_case_sensitive;

        match self.find_scope {
            FindScope::Scene => {
                if let Some(doc) = self.selected_doc.clone() {
                    let doc_title = self.active_document_title();
                    let chapter_title = match &doc {
                        SelectedDocument::Scene { chapter_idx, .. } => self
                            .project
                            .as_ref()
                            .and_then(|p| p.chapters.get(*chapter_idx))
                            .map(|c| c.title.clone()),
                        SelectedDocument::FrontMatter(_) => None,
                    };
                    for (b_start, b_end, c_start, c_end) in
                        find_all_occurrences(&self.editor_content, &query, case_sensitive)
                    {
                        self.find_matches.push(FindMatch {
                            doc: doc.clone(),
                            doc_title: doc_title.clone(),
                            chapter_title: chapter_title.clone(),
                            byte_start: b_start,
                            byte_end: b_end,
                            char_start: c_start,
                            char_end: c_end,
                        });
                    }
                }
            }
            FindScope::Chapter => {
                if let Some(SelectedDocument::Scene {
                    chapter_idx,
                    scene_idx: active_sc,
                }) = self.selected_doc
                {
                    if let Some(p) = &self.project {
                        if let Some(ch) = p.chapters.get(chapter_idx) {
                            for (sc_idx, sc) in ch.scenes.iter().enumerate() {
                                let disk_buf;
                                let text: &str = if sc_idx == active_sc {
                                    &self.editor_content
                                } else {
                                    disk_buf = sc.read_content().unwrap_or_default();
                                    &disk_buf
                                };
                                let doc = SelectedDocument::Scene {
                                    chapter_idx,
                                    scene_idx: sc_idx,
                                };
                                for (b_start, b_end, c_start, c_end) in
                                    find_all_occurrences(text, &query, case_sensitive)
                                {
                                    self.find_matches.push(FindMatch {
                                        doc: doc.clone(),
                                        doc_title: sc.title.clone(),
                                        chapter_title: Some(ch.title.clone()),
                                        byte_start: b_start,
                                        byte_end: b_end,
                                        char_start: c_start,
                                        char_end: c_end,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            FindScope::Manuscript => {
                if let Some(p) = &self.project {
                    for (ch_idx, ch) in p.chapters.iter().enumerate() {
                        for (sc_idx, sc) in ch.scenes.iter().enumerate() {
                            let is_active = matches!(
                                self.selected_doc,
                                Some(SelectedDocument::Scene {
                                    chapter_idx,
                                    scene_idx,
                                }) if chapter_idx == ch_idx && scene_idx == sc_idx
                            );
                            let disk_buf;
                            let text: &str = if is_active {
                                &self.editor_content
                            } else {
                                disk_buf = sc.read_content().unwrap_or_default();
                                &disk_buf
                            };
                            let doc = SelectedDocument::Scene {
                                chapter_idx: ch_idx,
                                scene_idx: sc_idx,
                            };
                            for (b_start, b_end, c_start, c_end) in
                                find_all_occurrences(text, &query, case_sensitive)
                            {
                                self.find_matches.push(FindMatch {
                                    doc: doc.clone(),
                                    doc_title: sc.title.clone(),
                                    chapter_title: Some(ch.title.clone()),
                                    byte_start: b_start,
                                    byte_end: b_end,
                                    char_start: c_start,
                                    char_end: c_end,
                                });
                            }
                        }
                    }
                }
            }
        }

        if self.find_matches.is_empty() {
            self.find_current_idx = 0;
            return;
        }

        if let Some((prev_doc, prev_byte)) = prev_key {
            if let Some(pos) = self
                .find_matches
                .iter()
                .position(|m| m.doc == prev_doc && m.byte_start == prev_byte)
            {
                self.find_current_idx = pos;
                return;
            }
        }

        if let Some(active_doc) = &self.selected_doc {
            if let Some(pos) = self.find_matches.iter().position(|m| &m.doc == active_doc) {
                self.find_current_idx = pos;
                return;
            }
        }

        self.find_current_idx = 0;
    }

    /// Navigates to the finding at `idx`, opening its scene if necessary and selecting it in the editor.
    pub fn go_to_match(&mut self, ctx: &egui::Context, idx: usize) {
        if idx >= self.find_matches.len() {
            return;
        }
        let target = self.find_matches[idx].clone();
        if self.selected_doc.as_ref() != Some(&target.doc) {
            match target.doc {
                SelectedDocument::FrontMatter(fm_idx) => self.select_front_matter(fm_idx),
                SelectedDocument::Scene {
                    chapter_idx,
                    scene_idx,
                } => self.select_scene(chapter_idx, scene_idx),
            }
        }
        self.find_current_idx = idx;
        self.find_scroll_to_match = true;

        let editor_id = egui::Id::new("central_editor_text");
        let mut state = egui::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(target.char_start),
                egui::text::CCursor::new(target.char_end),
            )));
        state.store(ctx, editor_id);
        ctx.request_repaint();
    }

    pub fn find_next_match(&mut self, ctx: &egui::Context) {
        if self.find_matches.is_empty() {
            return;
        }
        let next_idx = (self.find_current_idx + 1) % self.find_matches.len();
        self.go_to_match(ctx, next_idx);
    }

    pub fn find_prev_match(&mut self, ctx: &egui::Context) {
        if self.find_matches.is_empty() {
            return;
        }
        let prev_idx = if self.find_current_idx == 0 {
            self.find_matches.len() - 1
        } else {
            self.find_current_idx - 1
        };
        self.go_to_match(ctx, prev_idx);
    }

    /// Replaces only the currently selected finding with `self.find_replace_text`.
    pub fn replace_current_match(&mut self, ctx: &egui::Context) {
        if self.find_matches.is_empty() || self.find_query.is_empty() {
            return;
        }
        let idx = self.find_current_idx.min(self.find_matches.len() - 1);
        let m = self.find_matches[idx].clone();

        if self.selected_doc.as_ref() != Some(&m.doc) {
            match m.doc {
                SelectedDocument::FrontMatter(fm_idx) => self.select_front_matter(fm_idx),
                SelectedDocument::Scene {
                    chapter_idx,
                    scene_idx,
                } => self.select_scene(chapter_idx, scene_idx),
            }
        }

        if m.byte_end <= self.editor_content.len()
            && self.editor_content.is_char_boundary(m.byte_start)
            && self.editor_content.is_char_boundary(m.byte_end)
        {
            let replacement = self.find_replace_text.clone();
            self.editor_content
                .replace_range(m.byte_start..m.byte_end, &replacement);
            self.recompute_word_count();
            self.is_dirty = true;
            self.last_edit_time = Some(Instant::now());

            let next_byte_threshold = m.byte_start + replacement.len();
            let replaced_doc = m.doc.clone();
            self.refresh_find_matches();

            if !self.find_matches.is_empty() {
                let next_idx = self
                    .find_matches
                    .iter()
                    .position(|cand| {
                        cand.doc == replaced_doc && cand.byte_start >= next_byte_threshold
                    })
                    .unwrap_or(0);
                self.go_to_match(ctx, next_idx);
            }
            self.set_status("Changed 1 finding");
        }
    }

    /// Initiates "Change All": opens the confirmation modal when changing multiple findings,
    /// or applies directly if there is only a single finding.
    pub fn trigger_replace_all(&mut self) {
        let count = self.find_matches.len();
        if count == 0 || self.find_query.is_empty() {
            return;
        }
        if count > 1 {
            self.pending_replace_all_count = count;
            self.show_replace_confirm_modal = true;
        } else {
            self.replace_all_matches();
        }
    }

    /// Replaces all findings across the currently selected scope (`Scene`, `Chapter`, or `Manuscript`).
    pub fn replace_all_matches(&mut self) -> usize {
        if self.find_query.is_empty() {
            return 0;
        }

        let query = self.find_query.clone();
        let replacement = self.find_replace_text.clone();
        let case_sensitive = self.find_case_sensitive;
        let mut total_replaced = 0usize;

        match self.find_scope {
            FindScope::Scene => {
                if self.selected_doc.is_some() {
                    let (new_text, count) = replace_all_in_text(
                        &self.editor_content,
                        &query,
                        &replacement,
                        case_sensitive,
                    );
                    if count > 0 {
                        self.editor_content = new_text;
                        self.recompute_word_count();
                        self.is_dirty = true;
                        self.last_edit_time = Some(Instant::now());
                        total_replaced += count;
                    }
                }
            }
            FindScope::Chapter => {
                if let Some(SelectedDocument::Scene {
                    chapter_idx,
                    scene_idx: active_sc,
                }) = self.selected_doc
                {
                    let scene_paths: Vec<(usize, PathBuf)> = self
                        .project
                        .as_ref()
                        .and_then(|p| p.chapters.get(chapter_idx))
                        .map(|ch| {
                            ch.scenes
                                .iter()
                                .enumerate()
                                .map(|(i, s)| (i, s.path.clone()))
                                .collect()
                        })
                        .unwrap_or_default();

                    for (sc_idx, path) in scene_paths {
                        if sc_idx == active_sc {
                            let (new_text, count) = replace_all_in_text(
                                &self.editor_content,
                                &query,
                                &replacement,
                                case_sensitive,
                            );
                            if count > 0 {
                                self.editor_content = new_text;
                                self.recompute_word_count();
                                self.save_current_file();
                                total_replaced += count;
                            }
                        } else if let Ok(disk_text) = std::fs::read_to_string(&path) {
                            let (new_text, count) = replace_all_in_text(
                                &disk_text,
                                &query,
                                &replacement,
                                case_sensitive,
                            );
                            if count > 0 && std::fs::write(&path, new_text).is_ok() {
                                total_replaced += count;
                            }
                        }
                    }
                }
            }
            FindScope::Manuscript => {
                let all_scenes: Vec<(usize, usize, PathBuf)> = self
                    .project
                    .as_ref()
                    .map(|p| {
                        p.chapters
                            .iter()
                            .enumerate()
                            .flat_map(|(ch_idx, ch)| {
                                ch.scenes
                                    .iter()
                                    .enumerate()
                                    .map(move |(sc_idx, sc)| (ch_idx, sc_idx, sc.path.clone()))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                for (ch_idx, sc_idx, path) in all_scenes {
                    let is_active = matches!(
                        self.selected_doc,
                        Some(SelectedDocument::Scene {
                            chapter_idx,
                            scene_idx,
                        }) if chapter_idx == ch_idx && scene_idx == sc_idx
                    );
                    if is_active {
                        let (new_text, count) = replace_all_in_text(
                            &self.editor_content,
                            &query,
                            &replacement,
                            case_sensitive,
                        );
                        if count > 0 {
                            self.editor_content = new_text;
                            self.recompute_word_count();
                            self.save_current_file();
                            total_replaced += count;
                        }
                    } else if let Ok(disk_text) = std::fs::read_to_string(&path) {
                        let (new_text, count) =
                            replace_all_in_text(&disk_text, &query, &replacement, case_sensitive);
                        if count > 0 && std::fs::write(&path, new_text).is_ok() {
                            total_replaced += count;
                        }
                    }
                }
            }
        }

        self.refresh_find_matches();
        self.set_status(format!(
            "Changed {} finding{}",
            total_replaced,
            if total_replaced == 1 { "" } else { "s" }
        ));
        total_replaced
    }

    /// Renders the small Find & Change Findings pop-up anchored at the top-left of the page panel.
    pub(crate) fn render_find_popup(&mut self, ctx: &egui::Context) {
        if !self.show_find_popup {
            return;
        }

        let popup_pos = self
            .central_pane_rect
            .map(|r| r.left_top() + egui::vec2(12.0, 12.0))
            .unwrap_or_else(|| egui::pos2(260.0, 130.0));

        let mut close_popup = false;
        let mut go_next = false;
        let mut go_prev = false;
        let mut do_replace_one = false;
        let mut do_replace_all = false;

        let popup_bg = self.theme_format_bar_bg();
        let space_bg = self.theme_binder_bg();
        let border_col = self.theme_border();
        let text_dark = self.theme_text_dark();
        let text_muted = self.theme_text_muted();
        let accent_green = self.theme_accent_green();

        egui::Area::new(egui::Id::new("find_replace_popup_area"))
            .order(egui::Order::Foreground)
            .fixed_pos(popup_pos)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(popup_bg)
                    .stroke(egui::Stroke::new(1.0_f32, border_col))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(10))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 3],
                        blur: 10,
                        spread: 1,
                        color: egui::Color32::from_black_alpha(if self.dark_mode {
                            110
                        } else {
                            32
                        }),
                    })
                    .show(ui, |ui| {
                        ui.set_width(292.0);
                        ui.spacing_mut().item_spacing.y = 6.0;

                        // Header row: Title, match counter, navigation, Case button, Close
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("🔍 Find")
                                    .strong()
                                    .size(12.5)
                                    .color(text_dark),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;

                                    if ui
                                        .small_button(egui::RichText::new("✕").size(11.0))
                                        .on_hover_text("Close (Esc)")
                                        .clicked()
                                    {
                                        close_popup = true;
                                    }

                                    let case_btn = ui
                                        .selectable_label(
                                            self.find_case_sensitive,
                                            egui::RichText::new("Aa").size(11.0),
                                        )
                                        .on_hover_text("Match Case");
                                    if case_btn.clicked() {
                                        self.find_case_sensitive = !self.find_case_sensitive;
                                        self.refresh_find_matches();
                                        if !self.find_matches.is_empty() {
                                            self.go_to_match(ctx, self.find_current_idx);
                                        }
                                    }

                                    let has_matches = !self.find_matches.is_empty();
                                    if ui
                                        .add_enabled(
                                            has_matches,
                                            egui::Button::new(egui::RichText::new("▼").size(10.0))
                                                .small(),
                                        )
                                        .on_hover_text("Next Finding (F3 / Enter)")
                                        .clicked()
                                    {
                                        go_next = true;
                                    }
                                    if ui
                                        .add_enabled(
                                            has_matches,
                                            egui::Button::new(egui::RichText::new("▲").size(10.0))
                                                .small(),
                                        )
                                        .on_hover_text("Previous Finding (Shift+Enter)")
                                        .clicked()
                                    {
                                        go_prev = true;
                                    }

                                    if !self.find_query.is_empty() {
                                        let count_label = if self.find_matches.is_empty() {
                                            "0 found".to_string()
                                        } else {
                                            format!(
                                                "{}/{}",
                                                self.find_current_idx + 1,
                                                self.find_matches.len()
                                            )
                                        };
                                        ui.label(
                                            egui::RichText::new(count_label).size(11.0).color(
                                                if self.find_matches.is_empty() {
                                                    egui::Color32::from_rgb(190, 70, 70)
                                                } else {
                                                    text_muted
                                                },
                                            ),
                                        );
                                    }
                                },
                            );
                        });

                        // Space 1: Radiobox for selecting between Find in Scene, Chapter, Manuscript
                        egui::Frame::NONE
                            .fill(space_bg)
                            .stroke(egui::Stroke::new(0.5_f32, border_col))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(8, 5))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let prev_scope = self.find_scope;
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 3.0;
                                    ui.radio_value(
                                        &mut self.find_scope,
                                        FindScope::Scene,
                                        "Find in Scene",
                                    );
                                    ui.radio_value(
                                        &mut self.find_scope,
                                        FindScope::Chapter,
                                        "Find in Chapter",
                                    );
                                    ui.radio_value(
                                        &mut self.find_scope,
                                        FindScope::Manuscript,
                                        "Find in Manuscript",
                                    );
                                });
                                if self.find_scope != prev_scope {
                                    self.refresh_find_matches();
                                    if !self.find_matches.is_empty() {
                                        self.go_to_match(ctx, self.find_current_idx);
                                    }
                                }
                            });

                        // Space 2: Textbox for inputting the word being searched
                        egui::Frame::NONE
                            .fill(space_bg)
                            .stroke(egui::Stroke::new(0.5_f32, border_col))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                let search_resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.find_query)
                                        .id(egui::Id::new("find_query_input"))
                                        .hint_text("Word to find...")
                                        .desired_width(f32::INFINITY),
                                );

                                if self.find_focus_requested {
                                    search_resp.request_focus();
                                    self.find_focus_requested = false;
                                }

                                if search_resp.changed() {
                                    self.refresh_find_matches();
                                    if !self.find_matches.is_empty() {
                                        let idx = self.find_current_idx;
                                        let m = self.find_matches[idx].clone();
                                        if self.selected_doc.as_ref() == Some(&m.doc) {
                                            self.find_scroll_to_match = true;
                                            let editor_id = egui::Id::new("central_editor_text");
                                            let mut state = egui::text_edit::TextEditState::load(
                                                ctx, editor_id,
                                            )
                                            .unwrap_or_default();
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::two(
                                                    egui::text::CCursor::new(m.char_start),
                                                    egui::text::CCursor::new(m.char_end),
                                                ),
                                            ));
                                            state.store(ctx, editor_id);
                                        } else {
                                            self.go_to_match(ctx, idx);
                                        }
                                    }
                                }

                                if search_resp.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    if ui.input(|i| i.modifiers.shift) {
                                        go_prev = true;
                                    } else {
                                        go_next = true;
                                    }
                                    search_resp.request_focus();
                                }
                            });

                        // Location hint when searching Chapter or Manuscript
                        if self.find_scope != FindScope::Scene && !self.find_query.is_empty() {
                            if let Some(m) = self.find_matches.get(self.find_current_idx) {
                                let loc = match &m.chapter_title {
                                    Some(ch) => format!("📍 {} › {}", ch, m.doc_title),
                                    None => format!("📍 {}", m.doc_title),
                                };
                                ui.label(egui::RichText::new(loc).size(11.0).color(text_muted));
                            } else if self.find_scope == FindScope::Chapter
                                && !matches!(
                                    self.selected_doc,
                                    Some(SelectedDocument::Scene { .. })
                                )
                            {
                                ui.label(
                                    egui::RichText::new(
                                        "Select a chapter scene to search in Chapter.",
                                    )
                                    .size(11.0)
                                    .color(text_muted),
                                );
                            }
                        }

                        // Checkbox: "Change Findings" -> opens replacement textbox and buttons
                        ui.checkbox(&mut self.find_change_enabled, "Change Findings");

                        if self.find_change_enabled {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.find_replace_text)
                                    .id(egui::Id::new("find_replace_input"))
                                    .hint_text("Change findings into...")
                                    .desired_width(f32::INFINITY),
                            );

                            let match_count = self.find_matches.len();
                            let can_replace = match_count > 0 && !self.find_query.is_empty();

                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(can_replace, egui::Button::new("Change"))
                                    .on_hover_text("Change the current finding")
                                    .clicked()
                                {
                                    do_replace_one = true;
                                }

                                let all_label = if match_count > 0 {
                                    format!("Change All ({})", match_count)
                                } else {
                                    "Change All".to_string()
                                };
                                let all_btn = egui::Button::new(
                                    egui::RichText::new(all_label).color(if can_replace {
                                        egui::Color32::WHITE
                                    } else {
                                        text_muted
                                    }),
                                )
                                .fill(if can_replace { accent_green } else { space_bg });

                                if ui
                                    .add_enabled(can_replace, all_btn)
                                    .on_hover_text("Change all findings in the selected scope")
                                    .clicked()
                                {
                                    do_replace_all = true;
                                }
                            });
                        }
                    });
            });

        if go_next {
            self.find_next_match(ctx);
        } else if go_prev {
            self.find_prev_match(ctx);
        }

        if do_replace_one {
            self.replace_current_match(ctx);
        }

        if do_replace_all {
            self.trigger_replace_all();
        }

        if close_popup {
            self.show_find_popup = false;
        }
    }

    /// Renders the confirmation dialog before changing multiple findings at once.
    pub(crate) fn render_replace_confirm_modal(&mut self, ctx: &egui::Context) {
        if !self.show_replace_confirm_modal {
            return;
        }

        let mut close_modal = false;
        let mut confirm_replace = false;

        let scope_desc = match self.find_scope {
            FindScope::Scene => "the current scene",
            FindScope::Chapter => "the current chapter",
            FindScope::Manuscript => "the entire manuscript",
        };

        egui::Window::new("Confirm Change Findings")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.label(format!(
                    "Are you sure you want to change {} occurrences of \"{}\" into \"{}\" in {}?",
                    self.pending_replace_all_count,
                    self.find_query,
                    self.find_replace_text,
                    scope_desc,
                ));

                if self.find_replace_text.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "Note: The replacement text is empty, so matched words will be removed.",
                        )
                        .weak()
                        .size(11.5),
                    );
                }

                ui.separator();
                ui.horizontal(|ui| {
                    let confirm_btn = egui::Button::new(
                        egui::RichText::new(format!(
                            "Confirm Change ({})",
                            self.pending_replace_all_count
                        ))
                        .strong()
                        .color(egui::Color32::WHITE),
                    )
                    .fill(self.theme_accent_green());

                    if ui.add(confirm_btn).clicked() {
                        confirm_replace = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_modal = true;
                    }
                });
            });

        if confirm_replace {
            self.replace_all_matches();
            close_modal = true;
        }

        if close_modal {
            self.show_replace_confirm_modal = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::BookProject;
    use std::fs;

    #[test]
    fn test_find_all_occurrences_case_insensitive_and_sensitive() {
        let text = "The Shadow in the shadowland cast a SHADOW.";

        let ci = find_all_occurrences(text, "shadow", false);
        assert_eq!(ci.len(), 3);
        assert_eq!(&text[ci[0].0..ci[0].1], "Shadow");
        assert_eq!(&text[ci[1].0..ci[1].1], "shadow");
        assert_eq!(&text[ci[2].0..ci[2].1], "SHADOW");

        let cs = find_all_occurrences(text, "shadow", true);
        assert_eq!(cs.len(), 1);
        assert_eq!(&text[cs[0].0..cs[0].1], "shadow");
    }

    #[test]
    fn test_find_all_occurrences_unicode_offsets() {
        let text = "Café light — café glow, café!";
        let matches = find_all_occurrences(text, "café", false);
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0], (0, 5, 0, 4));
        assert_eq!(&text[matches[0].0..matches[0].1], "Café");
        assert_eq!(&text[matches[1].0..matches[1].1], "café");
        assert_eq!(&text[matches[2].0..matches[2].1], "café");
    }

    #[test]
    fn test_replace_all_in_text() {
        let text = "Elara looked at elara's reflection. ELARA smiled.";
        let (replaced, count) = replace_all_in_text(text, "elara", "Lyra", false);
        assert_eq!(count, 3);
        assert_eq!(replaced, "Lyra looked at Lyra's reflection. Lyra smiled.");

        let (unchanged, zero) = replace_all_in_text(text, "missing", "Lyra", false);
        assert_eq!(zero, 0);
        assert_eq!(unchanged, text);
    }

    #[test]
    fn test_apply_search_highlights_to_job_preserves_text_and_splits_sections() {
        let mut job = egui::text::LayoutJob::single_section(
            "Hello brave new world".to_string(),
            egui::TextFormat::default(),
        );
        apply_search_highlights_to_job(&mut job, "brave", false, Some(6), false);
        assert_eq!(job.text, "Hello brave new world");
        assert_eq!(job.sections.len(), 3);
        assert_eq!(job.sections[0].byte_range, 0..6);
        assert_eq!(job.sections[1].byte_range, 6..11);
        assert_eq!(
            job.sections[1].format.background,
            egui::Color32::from_rgb(245, 166, 35)
        );
        assert_eq!(job.sections[2].byte_range, 11..21);
    }

    #[test]
    fn test_find_and_change_findings_across_scopes() {
        let temp_dir = std::env::temp_dir().join("test_app_find_replace_scopes");
        let _ = fs::remove_dir_all(&temp_dir);

        let proj = BookProject::create_novel_blueprint(&temp_dir, "Find Test", "Author")
            .expect("create blueprint");

        let mut app = BookApp::default();
        app.set_project(proj);

        // Ch1 Sc1
        app.editor_content = "Aria walked into the lantern room. Aria paused.".to_string();
        app.is_dirty = true;
        app.save_current_file();

        // Ch1 Sc2
        app.add_scene();
        app.editor_content = "Outside, Aria heard the bell.".to_string();
        app.is_dirty = true;
        app.save_current_file();

        // Ch2 Sc1
        app.add_chapter();
        app.editor_content = "Days later, Aria returned to the city.".to_string();
        app.is_dirty = true;
        app.save_current_file();

        // Return to Ch1 Sc1
        app.select_scene(0, 0);
        app.editor_content = "Aria walked into the lantern room. Aria paused.".to_string();
        app.is_dirty = true;

        app.open_find_popup();
        assert!(app.show_find_popup);
        app.find_query = "Aria".to_string();

        // 1. Find in Scene -> 2 matches
        app.find_scope = FindScope::Scene;
        app.refresh_find_matches();
        assert_eq!(app.find_matches.len(), 2);

        // 2. Find in Chapter -> 3 matches
        app.find_scope = FindScope::Chapter;
        app.refresh_find_matches();
        assert_eq!(app.find_matches.len(), 3);

        // 3. Find in Manuscript -> 4 matches
        app.find_scope = FindScope::Manuscript;
        app.refresh_find_matches();
        assert_eq!(app.find_matches.len(), 4);

        // 4. Multiple matches -> trigger_replace_all opens confirmation modal
        app.find_change_enabled = true;
        app.find_replace_text = "Elena".to_string();
        app.trigger_replace_all();
        assert!(app.show_replace_confirm_modal);
        assert_eq!(app.pending_replace_all_count, 4);

        // Confirm replace all across Manuscript
        let replaced = app.replace_all_matches();
        app.show_replace_confirm_modal = false;
        assert_eq!(replaced, 4);
        assert_eq!(app.find_matches.len(), 0);
        assert_eq!(
            app.editor_content,
            "Elena walked into the lantern room. Elena paused."
        );

        // Verify Ch1 Sc2 and Ch2 Sc1 on disk were updated
        let p = app.project.as_ref().unwrap();
        let ch1_sc2 = p.chapters[0].scenes[1].read_content().unwrap();
        let ch2_sc1 = p.chapters[1].scenes[0].read_content().unwrap();
        assert_eq!(ch1_sc2, "Outside, Elena heard the bell.");
        assert_eq!(ch2_sc1, "Days later, Elena returned to the city.");

        // 5. Single match -> trigger_replace_all replaces without confirmation modal
        app.find_scope = FindScope::Scene;
        app.find_query = "lantern".to_string();
        app.find_replace_text = "candlelit".to_string();
        app.refresh_find_matches();
        assert_eq!(app.find_matches.len(), 1);
        app.trigger_replace_all();
        assert!(!app.show_replace_confirm_modal);
        assert_eq!(
            app.editor_content,
            "Elena walked into the candlelit room. Elena paused."
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_popup_shortcuts_ctrl_f_and_f3() {
        let ctx = egui::Context::default();
        let mut app = BookApp::default();
        app.editor_content = "Alpha beta alpha".to_string();
        app.selected_doc = Some(SelectedDocument::FrontMatter(0));

        // Ctrl+F opens popup
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::F,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::CTRL,
        });
        let _ = ctx.run(input, |ctx| {
            app.handle_shortcuts(ctx);
        });
        assert!(app.show_find_popup);

        // Escape closes popup
        let mut esc_input = egui::RawInput::default();
        esc_input.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(esc_input, |ctx| {
            app.handle_shortcuts(ctx);
        });
        assert!(!app.show_find_popup);

        // F3 opens popup when closed
        let mut f3_input = egui::RawInput::default();
        f3_input.events.push(egui::Event::Key {
            key: egui::Key::F3,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(f3_input, |ctx| {
            app.handle_shortcuts(ctx);
        });
        assert!(app.show_find_popup);

        // F3 advances to next finding when popup is already open
        app.find_query = "alpha".to_string();
        app.refresh_find_matches();
        assert_eq!(app.find_matches.len(), 2);
        assert_eq!(app.find_current_idx, 0);

        let mut f3_next_input = egui::RawInput::default();
        f3_next_input.events.push(egui::Event::Key {
            key: egui::Key::F3,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(f3_next_input, |ctx| {
            app.handle_shortcuts(ctx);
        });
        assert_eq!(app.find_current_idx, 1);
    }

    #[test]
    fn test_replace_current_match_cross_scene_navigation_and_ui_render() {
        let temp_dir = std::env::temp_dir().join("test_app_find_replace_ui_render");
        let _ = fs::remove_dir_all(&temp_dir);

        let proj = BookProject::create_novel_blueprint(&temp_dir, "UI Test", "Author")
            .expect("create blueprint");

        let mut app = BookApp::default();
        app.set_project(proj);
        app.editor_content = "First torch, second torch.".to_string();
        app.save_current_file();

        app.add_scene();
        app.editor_content = "Third torch and fourth torch in scene 2.".to_string();
        app.save_current_file();

        app.select_scene(0, 0);
        app.open_find_popup();
        app.find_scope = FindScope::Manuscript;
        app.find_query = "torch".to_string();
        app.find_change_enabled = true;
        app.find_replace_text = "lantern".to_string();
        app.refresh_find_matches();
        assert_eq!(app.find_matches.len(), 4);

        let ctx = egui::Context::default();
        // Navigate across scenes using go_to_match
        app.go_to_match(&ctx, 2);
        assert_eq!(
            app.selected_doc,
            Some(SelectedDocument::Scene {
                chapter_idx: 0,
                scene_idx: 1,
            })
        );
        assert_eq!(app.editor_content, "Third torch and fourth torch in scene 2.");

        // Replace current finding in scene 2 -> advances to 4th torch in same scene
        app.replace_current_match(&ctx);
        assert_eq!(
            app.editor_content,
            "Third lantern and fourth torch in scene 2."
        );
        assert_eq!(app.find_matches.len(), 3);

        // Replace last finding in scene 2 -> saves scene 2 and wraps to scene 1
        app.replace_current_match(&ctx);
        assert_eq!(app.find_matches.len(), 2);
        assert_eq!(
            app.selected_doc,
            Some(SelectedDocument::Scene {
                chapter_idx: 0,
                scene_idx: 0,
            })
        );
        let sc2_disk = app
            .project
            .as_ref()
            .unwrap()
            .chapters[0]
            .scenes[1]
            .read_content()
            .unwrap();
        assert_eq!(sc2_disk, "Third lantern and fourth lantern in scene 2.");

        // Render popup and confirmation modal in both light and dark modes without panicking
        app.trigger_replace_all();
        assert!(app.show_replace_confirm_modal);

        for dark in [false, true] {
            app.dark_mode = dark;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.central_pane_rect = Some(ui.max_rect());
                });
                app.render_find_popup(ctx);
                app.render_replace_confirm_modal(ctx);
            });
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
