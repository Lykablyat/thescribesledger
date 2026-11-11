use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use crate::compiler::{is_boilerplate_scene_heading, replace_placeholders};
use crate::project::{BookProject, ChapterKind};

/// Compiles a `BookProject` into a formatted PDF file.
pub fn compile_book_to_pdf<P: AsRef<Path>>(project: &BookProject, output_path: P) -> Result<()> {
    let mut doc = SimplePdf::new(&project.title, &project.author);

    // 1. Title Page (first page)
    doc.start_page();
    doc.add_vertical_space(180.0);
    doc.add_heading_1(&project.title, true);
    doc.add_vertical_space(20.0);
    doc.add_heading_2(&format!("By {}", project.author), true);
    doc.add_vertical_space(40.0);
    doc.add_italic_text("*Draft Edition*", true);

    // 2. Front matter files (Copyright, Dedication, etc.)
    for item in &project.front_matter {
        if item.name.contains("title_page") || item.name.contains("Title Page") {
            continue;
        }
        if let Ok(content) = item.read_content() {
            let replaced = replace_placeholders(&content, &project.title, &project.author);
            doc.start_page();
            doc.render_markdown(&replaced);
        }
    }

    // 3. Manuscript chapters and scenes
    let mut chapter_number: usize = 0;
    for chapter in &project.chapters {
        doc.start_page();
        let heading = match &chapter.kind {
            ChapterKind::Numbered => {
                chapter_number += 1;
                chapter_number.to_string()
            }
            ChapterKind::Specialized(special_title) => special_title.clone(),
        };
        doc.add_heading_1(&heading, true);
        doc.add_vertical_space(15.0);

        for (scene_idx, scene) in chapter.scenes.iter().enumerate() {
            if scene_idx > 0 {
                doc.add_vertical_space(10.0);
                doc.add_body_text("* * *", true);
                doc.add_vertical_space(10.0);
            }
            if let Ok(content) = scene.read_content() {
                let replaced = replace_placeholders(&content, &project.title, &project.author);
                let mut filtered = String::with_capacity(replaced.len());
                for line in replaced.lines() {
                    if !is_boilerplate_scene_heading(line.trim()) {
                        filtered.push_str(line);
                        filtered.push('\n');
                    }
                }
                doc.render_markdown(&filtered);
                doc.add_vertical_space(15.0);
            }
        }
    }

    let pdf_bytes = doc.build();
    let path = output_path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, pdf_bytes)
        .with_context(|| format!("Failed to write PDF output to {:?}", path))?;

    Ok(())
}

/// Lightweight, pure-Rust PDF 1.4 document builder.
struct SimplePdf {
    title: String,
    author: String,
    pages: Vec<PdfPage>,
}

struct PdfPage {
    commands: Vec<String>,
    current_y: f32,
}

const PAGE_WIDTH: f32 = 595.28;  // A4 width in pt
const PAGE_HEIGHT: f32 = 841.89; // A4 height in pt
const MARGIN_LEFT: f32 = 54.0;
const MARGIN_RIGHT: f32 = 54.0;
const MARGIN_TOP: f32 = 780.0;
const MARGIN_BOTTOM: f32 = 54.0;

impl SimplePdf {
    pub fn new(title: &str, author: &str) -> Self {
        Self {
            title: title.to_string(),
            author: author.to_string(),
            pages: Vec::new(),
        }
    }

    pub fn start_page(&mut self) {
        self.pages.push(PdfPage {
            commands: Vec::new(),
            current_y: MARGIN_TOP,
        });
    }

    fn current_page_mut(&mut self) -> &mut PdfPage {
        if self.pages.is_empty() {
            self.start_page();
        }
        self.pages.last_mut().unwrap()
    }

    pub fn add_vertical_space(&mut self, amount: f32) {
        let page = self.current_page_mut();
        page.current_y -= amount;
        if page.current_y < MARGIN_BOTTOM {
            self.start_page();
        }
    }

    pub fn add_heading_1(&mut self, text: &str, centered: bool) {
        self.render_line(text, 22.0, "F2", centered, 32.0);
    }

    pub fn add_heading_2(&mut self, text: &str, centered: bool) {
        self.render_line(text, 16.0, "F2", centered, 24.0);
    }

    pub fn add_body_text(&mut self, text: &str, centered: bool) {
        self.render_line(text, 11.0, "F1", centered, 16.0);
    }

    pub fn add_italic_text(&mut self, text: &str, centered: bool) {
        self.render_line(text, 11.0, "F3", centered, 16.0);
    }

    fn render_line(&mut self, text: &str, font_size: f32, font_ref: &str, centered: bool, line_height: f32) {
        if self.current_page_mut().current_y - line_height < MARGIN_BOTTOM {
            self.start_page();
        }

        let y = self.current_page_mut().current_y;
        self.current_page_mut().current_y -= line_height;

        let clean_text = clean_pdf_string(text);
        let approx_char_width = font_size * 0.52;
        let text_width = clean_text.len() as f32 * approx_char_width;

        let x = if centered {
            ((PAGE_WIDTH - text_width) / 2.0).max(MARGIN_LEFT)
        } else {
            MARGIN_LEFT
        };

        let cmd = format!(
            "BT /{font} {size} Tf 1 0 0 1 {x:.2} {y:.2} Tm ({text}) Tj ET",
            font = font_ref,
            size = font_size,
            x = x,
            y = y,
            text = clean_text
        );
        self.current_page_mut().commands.push(cmd);
    }

    pub fn render_markdown(&mut self, markdown: &str) {
        let printable_width = PAGE_WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
        let max_line_len = (printable_width / 6.5) as usize;

        let mut current_line = String::with_capacity(max_line_len + 16);

        for raw_line in markdown.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                self.add_vertical_space(10.0);
                continue;
            }

            if let Some(rest) = line.strip_prefix("# ") {
                self.add_heading_1(rest.trim(), false);
            } else if let Some(rest) = line.strip_prefix("## ") {
                self.add_heading_2(rest.trim(), false);
            } else if let Some(rest) = line.strip_prefix("### ") {
                self.render_line(rest.trim(), 13.0, "F2", false, 20.0);
            } else if line.starts_with('*') && line.ends_with('*') && line.len() > 2 {
                let text = &line[1..line.len() - 1];
                self.add_italic_text(text, false);
            } else {
                // Stream words without allocating a Vec<&str>
                current_line.clear();
                for word in line.split_whitespace() {
                    if !current_line.is_empty() && current_line.len() + word.len() + 1 > max_line_len {
                        self.add_body_text(&current_line, false);
                        current_line.clear();
                    }
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(word);
                }
                if !current_line.is_empty() {
                    self.add_body_text(&current_line, false);
                }
            }
        }
    }

    pub fn build(self) -> Vec<u8> {
        let mut pdf = Vec::with_capacity(self.pages.len() * 2048 + 1024);
        pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

        let total_pages = self.pages.len().max(1);
        let mut offsets: Vec<usize> = Vec::with_capacity(6 + total_pages * 2);
        offsets.push(0); // dummy for 0 0 R

        // Object 1: Catalog
        offsets.push(pdf.len());
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // Object 2: Pages container
        offsets.push(pdf.len());
        let mut kids = String::with_capacity(total_pages * 8);
        for i in 0..total_pages {
            let page_obj_id = 6 + i * 2;
            kids.push_str(&format!("{} 0 R ", page_obj_id));
        }
        let pages_obj = format!(
            "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
            kids.trim(),
            total_pages
        );
        pdf.extend_from_slice(pages_obj.as_bytes());

        // Object 3: Helvetica (regular)
        offsets.push(pdf.len());
        pdf.extend_from_slice(b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n");

        // Object 4: Helvetica-Bold
        offsets.push(pdf.len());
        pdf.extend_from_slice(b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>\nendobj\n");

        // Object 5: Helvetica-Oblique
        offsets.push(pdf.len());
        pdf.extend_from_slice(b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Oblique /Encoding /WinAnsiEncoding >>\nendobj\n");

        // Pages and contents
        for (i, page) in self.pages.iter().enumerate() {
            let page_obj_id = 6 + i * 2;
            let stream_obj_id = page_obj_id + 1;

            // Page object
            offsets.push(pdf.len());
            let page_entry = format!(
                "{id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Contents {s_id} 0 R /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >> >> >>\nendobj\n",
                id = page_obj_id,
                w = PAGE_WIDTH,
                h = PAGE_HEIGHT,
                s_id = stream_obj_id
            );
            pdf.extend_from_slice(page_entry.as_bytes());

            // Content stream
            let mut stream_data = String::with_capacity(page.commands.len() * 64 + 64);
            for cmd in &page.commands {
                stream_data.push_str(cmd);
                stream_data.push('\n');
            }
            if i > 0 {
                let footer = format!(
                    "BT /F1 9 Tf 1 0 0 1 {x:.2} 36.00 Tm ({page_num}) Tj ET\n",
                    x = PAGE_WIDTH / 2.0 - 5.0,
                    page_num = i + 1
                );
                stream_data.push_str(&footer);
            }

            offsets.push(pdf.len());
            let stream_obj = format!(
                "{id} 0 obj\n<< /Length {len} >>\nstream\n{data}endstream\nendobj\n",
                id = stream_obj_id,
                len = stream_data.len(),
                data = stream_data
            );
            pdf.extend_from_slice(stream_obj.as_bytes());
        }

        // XRef Table
        let xref_start = pdf.len();
        let total_objs = offsets.len();
        pdf.extend_from_slice(format!("xref\n0 {}\n", total_objs).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
        }

        // Trailer
        let trailer = format!(
            "trailer\n<< /Size {size} /Root 1 0 R /Info << /Title ({title}) /Author ({author}) >> >>\nstartxref\n{xref_start}\n%%EOF\n",
            size = total_objs,
            title = clean_pdf_string(&self.title),
            author = clean_pdf_string(&self.author),
            xref_start = xref_start
        );
        pdf.extend_from_slice(trailer.as_bytes());

        pdf
    }
}

fn clean_pdf_string(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for c in input.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' | '\n' => out.push(' '),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            '’' | '‘' => out.push('\''),
            '“' | '”' => out.push('"'),
            '—' | '–' => out.push('-'),
            '…' => out.push_str("..."),
            _ => out.push('?'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_compile_book_to_pdf() {
        let temp_dir = std::env::temp_dir().join("test_book_compile");
        let _ = fs::remove_dir_all(&temp_dir);

        let project = BookProject::create_novel_blueprint(&temp_dir, "My Novel", "Author Name")
            .expect("create blueprint");

        let pdf_out = temp_dir.join("novel.pdf");
        compile_book_to_pdf(&project, &pdf_out).expect("compile pdf");

        assert!(pdf_out.exists(), "pdf file should be created");
        let bytes = fs::read(&pdf_out).expect("read pdf");
        assert!(bytes.starts_with(b"%PDF-1.4"), "Valid PDF header");
        assert!(bytes.ends_with(b"%%EOF\n") || bytes.ends_with(b"%%EOF"), "Valid PDF trailer");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
