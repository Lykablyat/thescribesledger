use anyhow::{anyhow, Context, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Category grouping for font dropdowns and menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontCategory {
    Serif,
    SansSerif,
    Monospace,
    Custom,
}

impl FontCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Serif => "Serif (Book & Literary)",
            Self::SansSerif => "Sans-Serif",
            Self::Monospace => "Monospace (Draft & Script)",
            Self::Custom => "Imported TTF Fonts",
        }
    }
}

/// Font family choices available across the editor, profiles, and PDF compiler.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontFamilyChoice {
    // Original variants preserved for full TOML backwards compatibility
    Serif,
    Proportional,
    Monospace,

    // Widely-used Serif fonts
    Georgia,
    TimesNewRoman,
    Garamond,
    Palatino,
    BookAntiqua,
    Century,
    Constantia,

    // Widely-used Sans-Serif fonts (including Helvetica)
    Helvetica,
    Arial,
    Calibri,
    Verdana,

    // Widely-used Monospace fonts
    CourierNew,

    // User-imported singular TTF font
    Custom(String),
}

impl FontFamilyChoice {
    /// Ordered list of all built-in font families grouped cleanly by style.
    pub fn builtin_choices() -> &'static [FontFamilyChoice] {
        &[
            // Serif
            Self::Serif,
            Self::Georgia,
            Self::TimesNewRoman,
            Self::Garamond,
            Self::Palatino,
            Self::BookAntiqua,
            Self::Century,
            Self::Constantia,
            // Sans-Serif
            Self::Helvetica,
            Self::Arial,
            Self::Calibri,
            Self::Proportional,
            Self::Verdana,
            // Monospace
            Self::Monospace,
            Self::CourierNew,
        ]
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Serif => "Sitka Text",
            Self::Proportional => "Segoe UI",
            Self::Monospace => "Consolas",
            Self::Georgia => "Georgia",
            Self::TimesNewRoman => "Times New Roman",
            Self::Garamond => "Garamond",
            Self::Palatino => "Palatino Linotype",
            Self::BookAntiqua => "Book Antiqua",
            Self::Century => "Century",
            Self::Constantia => "Constantia",
            Self::Helvetica => "Helvetica",
            Self::Arial => "Arial",
            Self::Calibri => "Calibri",
            Self::Verdana => "Verdana",
            Self::CourierNew => "Courier New",
            Self::Custom(name) => name.as_str(),
        }
    }

    pub fn category(&self) -> FontCategory {
        match self {
            Self::Serif
            | Self::Georgia
            | Self::TimesNewRoman
            | Self::Garamond
            | Self::Palatino
            | Self::BookAntiqua
            | Self::Century
            | Self::Constantia => FontCategory::Serif,
            Self::Proportional | Self::Helvetica | Self::Arial | Self::Calibri | Self::Verdana => {
                FontCategory::SansSerif
            }
            Self::Monospace | Self::CourierNew => FontCategory::Monospace,
            Self::Custom(_) => FontCategory::Custom,
        }
    }

    pub fn menu_label(&self) -> String {
        match self.category() {
            FontCategory::Serif => format!("{} (Serif)", self.name()),
            FontCategory::SansSerif => format!("{} (Sans)", self.name()),
            FontCategory::Monospace => format!("{} (Mono)", self.name()),
            FontCategory::Custom => format!("{} (Imported TTF)", self.name()),
        }
    }

    /// Key used inside `egui::FontFamily::Name(...)` when registered.
    pub fn egui_family_key(&self) -> Option<String> {
        match self {
            Self::Serif => Some("Serif".to_string()),
            Self::Proportional | Self::Monospace => None,
            Self::Georgia => Some("builtin:Georgia".to_string()),
            Self::TimesNewRoman => Some("builtin:TimesNewRoman".to_string()),
            Self::Garamond => Some("builtin:Garamond".to_string()),
            Self::Palatino => Some("builtin:Palatino".to_string()),
            Self::BookAntiqua => Some("builtin:BookAntiqua".to_string()),
            Self::Century => Some("builtin:Century".to_string()),
            Self::Constantia => Some("builtin:Constantia".to_string()),
            Self::Helvetica => Some("builtin:Helvetica".to_string()),
            Self::Arial => Some("builtin:Arial".to_string()),
            Self::Calibri => Some("builtin:Calibri".to_string()),
            Self::Verdana => Some("builtin:Verdana".to_string()),
            Self::CourierNew => Some("builtin:CourierNew".to_string()),
            Self::Custom(name) => Some(format!("custom:{name}")),
        }
    }

    /// Resolves to a safe `egui::FontFamily`, falling back cleanly if a custom/system font isn't registered.
    pub fn to_egui_family(&self, registered_families: &HashSet<String>) -> egui::FontFamily {
        match self {
            Self::Proportional => egui::FontFamily::Proportional,
            Self::Monospace => egui::FontFamily::Monospace,
            Self::Serif => {
                if registered_families.contains("Serif") {
                    egui::FontFamily::Name("Serif".into())
                } else {
                    egui::FontFamily::Proportional
                }
            }
            other => {
                if let Some(key) = other.egui_family_key() {
                    if registered_families.contains(&key) {
                        return egui::FontFamily::Name(key.into());
                    }
                }
                if other.category() == FontCategory::Monospace {
                    egui::FontFamily::Monospace
                } else {
                    egui::FontFamily::Proportional
                }
            }
        }
    }

    /// Returns Typst font family fallback list for PDF compilation.
    pub fn typst_fallbacks(&self, imported_fonts: &[ImportedFont]) -> Vec<String> {
        match self {
            Self::Serif => vec![
                "Sitka Text".to_string(),
                "Libertinus Serif".to_string(),
                "Georgia".to_string(),
            ],
            Self::Proportional => vec!["Segoe UI".to_string(), "Arial".to_string()],
            Self::Monospace => vec!["Consolas".to_string(), "Courier New".to_string()],
            Self::Georgia => vec!["Georgia".to_string(), "Libertinus Serif".to_string()],
            Self::TimesNewRoman => {
                vec![
                    "Times New Roman".to_string(),
                    "Libertinus Serif".to_string(),
                ]
            }
            Self::Garamond => vec!["Garamond".to_string(), "Libertinus Serif".to_string()],
            Self::Palatino => vec![
                "Palatino Linotype".to_string(),
                "Book Antiqua".to_string(),
                "Libertinus Serif".to_string(),
            ],
            Self::BookAntiqua => vec![
                "Book Antiqua".to_string(),
                "Palatino Linotype".to_string(),
                "Libertinus Serif".to_string(),
            ],
            Self::Century => vec!["Century".to_string(), "Libertinus Serif".to_string()],
            Self::Constantia => vec!["Constantia".to_string(), "Libertinus Serif".to_string()],
            Self::Helvetica => vec!["Helvetica".to_string(), "Arial".to_string()],
            Self::Arial => vec!["Arial".to_string(), "Helvetica".to_string()],
            Self::Calibri => vec!["Calibri".to_string(), "Arial".to_string()],
            Self::Verdana => vec!["Verdana".to_string(), "Arial".to_string()],
            Self::CourierNew => vec!["Courier New".to_string(), "Consolas".to_string()],
            Self::Custom(name) => {
                if let Some(imp) = imported_fonts.iter().find(|f| f.name == *name) {
                    vec![imp.typst_family.clone(), "Libertinus Serif".to_string()]
                } else {
                    vec![name.clone(), "Libertinus Serif".to_string()]
                }
            }
        }
    }
}

/// Metadata for a user-imported singular `.ttf` font file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedFont {
    pub name: String,
    pub typst_family: String,
    pub path: PathBuf,
}

/// Returns candidate file paths for each built-in expandable font family.
fn candidate_paths_for_builtin(choice: &FontFamilyChoice) -> Vec<PathBuf> {
    let win_fonts = PathBuf::from(r"C:\Windows\Fonts");
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("Microsoft").join("Windows").join("Fonts"));

    let mut out = Vec::new();
    let add_win = |list: &mut Vec<PathBuf>, files: &[&str]| {
        for f in files {
            if let Some(user_dir) = &local_app_data {
                list.push(user_dir.join(f));
            }
            list.push(win_fonts.join(f));
        }
    };

    match choice {
        FontFamilyChoice::Serif => {
            add_win(&mut out, &["georgia.ttf", "times.ttf"]);
        }
        FontFamilyChoice::Georgia => {
            add_win(&mut out, &["georgia.ttf"]);
            out.push(PathBuf::from("/Library/Fonts/Georgia.ttf"));
        }
        FontFamilyChoice::TimesNewRoman => {
            add_win(&mut out, &["times.ttf"]);
            out.push(PathBuf::from("/Library/Fonts/Times New Roman.ttf"));
        }
        FontFamilyChoice::Garamond => {
            add_win(
                &mut out,
                &["GARA.TTF", "gara.ttf", "EBGaramond-Regular.ttf"],
            );
        }
        FontFamilyChoice::Palatino => {
            add_win(&mut out, &["pala.ttf", "BKANT.TTF"]);
        }
        FontFamilyChoice::BookAntiqua => {
            add_win(&mut out, &["BKANT.TTF", "bkant.ttf", "pala.ttf"]);
        }
        FontFamilyChoice::Century => {
            add_win(&mut out, &["CENTURY.TTF", "century.ttf"]);
        }
        FontFamilyChoice::Constantia => {
            add_win(&mut out, &["constan.ttf"]);
        }
        FontFamilyChoice::Helvetica => {
            add_win(
                &mut out,
                &[
                    "Helvetica.ttf",
                    "helvetica.ttf",
                    "HelveticaNeue.ttf",
                    "arial.ttf",
                ],
            );
            out.push(PathBuf::from(
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            ));
        }
        FontFamilyChoice::Arial => {
            add_win(&mut out, &["arial.ttf"]);
            out.push(PathBuf::from("/Library/Fonts/Arial.ttf"));
        }
        FontFamilyChoice::Calibri => {
            add_win(&mut out, &["calibri.ttf", "arial.ttf"]);
        }
        FontFamilyChoice::Verdana => {
            add_win(&mut out, &["verdana.ttf", "arial.ttf"]);
        }
        FontFamilyChoice::CourierNew => {
            add_win(&mut out, &["cour.ttf", "consola.ttf"]);
        }
        FontFamilyChoice::Proportional
        | FontFamilyChoice::Monospace
        | FontFamilyChoice::Custom(_) => {}
    }

    out
}

/// Validates that `bytes` looks like a TrueType/OpenType font file (SFNT container)
/// and extracts the preferred font family name from its OpenType `name` table.
pub fn parse_ttf_family_name(bytes: &[u8]) -> Result<Option<String>> {
    if bytes.len() < 12 {
        return Err(anyhow!("File is too small to be a valid TTF font"));
    }

    let sfnt = &bytes[0..4];
    let valid_sfnt =
        sfnt == [0x00, 0x01, 0x00, 0x00] || sfnt == b"true" || sfnt == b"OTTO" || sfnt == b"typ1";
    if !valid_sfnt {
        return Err(anyhow!(
            "Unsupported font header; expected a valid TrueType (.ttf) font file"
        ));
    }

    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    if bytes.len() < 12 + num_tables * 16 {
        return Err(anyhow!("Corrupted SFNT table directory in font file"));
    }

    let mut name_table: Option<(usize, usize)> = None;
    let mut has_head = false;
    for i in 0..num_tables {
        let rec_off = 12 + i * 16;
        let tag = &bytes[rec_off..rec_off + 4];
        let tbl_off = u32::from_be_bytes([
            bytes[rec_off + 8],
            bytes[rec_off + 9],
            bytes[rec_off + 10],
            bytes[rec_off + 11],
        ]) as usize;
        let tbl_len = u32::from_be_bytes([
            bytes[rec_off + 12],
            bytes[rec_off + 13],
            bytes[rec_off + 14],
            bytes[rec_off + 15],
        ]) as usize;

        if tag == b"head" {
            has_head = true;
        } else if tag == b"name" {
            name_table = Some((tbl_off, tbl_len));
        }
    }

    if !has_head {
        return Err(anyhow!("Font file is missing required 'head' table"));
    }

    let Some((tbl_off, tbl_len)) = name_table else {
        return Ok(None);
    };
    if tbl_len < 6 || tbl_off.saturating_add(tbl_len) > bytes.len() {
        return Ok(None);
    }

    let name_bytes = &bytes[tbl_off..tbl_off + tbl_len];
    let count = u16::from_be_bytes([name_bytes[2], name_bytes[3]]) as usize;
    let string_offset = u16::from_be_bytes([name_bytes[4], name_bytes[5]]) as usize;

    if 6 + count * 12 > name_bytes.len() || string_offset > name_bytes.len() {
        return Ok(None);
    }

    let mut best_name: Option<(u8, String)> = None;

    for i in 0..count {
        let rec = &name_bytes[6 + i * 12..6 + (i + 1) * 12];
        let platform_id = u16::from_be_bytes([rec[0], rec[1]]);
        let name_id = u16::from_be_bytes([rec[6], rec[7]]);
        let length = u16::from_be_bytes([rec[8], rec[9]]) as usize;
        let str_off = u16::from_be_bytes([rec[10], rec[11]]) as usize;

        // nameID 16 = Typographic Family, nameID 1 = Font Family
        if name_id != 16 && name_id != 1 {
            continue;
        }

        let start = string_offset.saturating_add(str_off);
        let end = start.saturating_add(length);
        if end > name_bytes.len() || length == 0 {
            continue;
        }

        let raw = &name_bytes[start..end];
        let decoded = match platform_id {
            0 | 3 => {
                if raw.len() % 2 != 0 {
                    continue;
                }
                let u16s: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                char::decode_utf16(u16s)
                    .filter_map(|r| r.ok())
                    .collect::<String>()
            }
            1 => raw.iter().map(|&b| b as char).collect::<String>(),
            _ => continue,
        };

        let cleaned = decoded
            .trim_matches(|c: char| c == '\0' || c.is_whitespace())
            .to_string();
        if cleaned.is_empty() {
            continue;
        }

        // Priority score: prefer Typographic Family (16) + Windows (3), then Font Family (1) + Windows (3)
        let score = match (name_id, platform_id) {
            (16, 3) => 4,
            (16, _) => 3,
            (1, 3) => 2,
            _ => 1,
        };
        if best_name.as_ref().is_none_or(|(best_s, _)| score > *best_s) {
            best_name = Some((score, cleaned));
        }
    }

    Ok(best_name.map(|(_, s)| s))
}

/// Imports a singular `.ttf` font file, copies it into `storage_dir`, parses its internal
/// Typst/display family name, and returns the resulting `ImportedFont` record.
pub fn import_ttf_font(
    source_path: &Path,
    storage_dir: &Path,
    existing: &[ImportedFont],
) -> Result<ImportedFont> {
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "ttf" {
        return Err(anyhow!(
            "Please select a singular TrueType (.ttf) font file"
        ));
    }

    let bytes = fs::read(source_path)
        .with_context(|| format!("Failed to read font file {:?}", source_path))?;

    let parsed_family = parse_ttf_family_name(&bytes)?;
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("CustomFont")
        .trim()
        .to_string();

    let typst_family = parsed_family.clone().unwrap_or_else(|| stem.clone());
    let base_display = parsed_family.unwrap_or_else(|| stem.clone());

    // Ensure unique display name across built-in and previously imported fonts
    let mut display_name = base_display.clone();
    let mut suffix = 2usize;
    while FontFamilyChoice::builtin_choices()
        .iter()
        .any(|b| b.name().eq_ignore_ascii_case(&display_name))
        || existing
            .iter()
            .any(|e| e.name.eq_ignore_ascii_case(&display_name))
    {
        display_name = format!("{} ({})", base_display, suffix);
        suffix += 1;
    }

    fs::create_dir_all(storage_dir)
        .with_context(|| format!("Failed to create fonts directory {:?}", storage_dir))?;

    let safe_stem: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe_stem = if safe_stem.is_empty() {
        "imported_font".to_string()
    } else {
        safe_stem
    };

    let mut dest_path = storage_dir.join(format!("{safe_stem}.ttf"));
    let mut file_idx = 2usize;
    while dest_path.exists() {
        dest_path = storage_dir.join(format!("{safe_stem}_{file_idx}.ttf"));
        file_idx += 1;
    }

    fs::write(&dest_path, &bytes)
        .with_context(|| format!("Failed to copy font file to {:?}", dest_path))?;

    Ok(ImportedFont {
        name: display_name,
        typst_family,
        path: dest_path,
    })
}

/// Registers built-in expanded fonts and all user-imported `.ttf` fonts with `egui::Context`.
/// Returns the set of `FontFamily::Name` keys successfully registered.
pub fn register_egui_fonts(
    ctx: &egui::Context,
    imported_fonts: &[ImportedFont],
) -> HashSet<String> {
    let mut fonts = egui::FontDefinitions::default();
    let mut registered = HashSet::new();

    let default_prop = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let default_mono = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    for choice in FontFamilyChoice::builtin_choices() {
        let Some(family_key) = choice.egui_family_key() else {
            continue;
        };

        let mut loaded_data_key: Option<String> = None;
        for cand in candidate_paths_for_builtin(choice) {
            if let Ok(bytes) = fs::read(&cand) {
                let data_key = if family_key == "Serif" {
                    "system_serif".to_string()
                } else {
                    format!("data:{}", family_key)
                };
                fonts.font_data.insert(
                    data_key.clone(),
                    Arc::new(egui::FontData::from_owned(bytes)),
                );
                loaded_data_key = Some(data_key);
                break;
            }
        }

        let mut family_list = Vec::new();
        if let Some(dk) = loaded_data_key {
            family_list.push(dk);
        } else if family_key != "Serif" {
            continue;
        }

        if choice.category() == FontCategory::Monospace {
            family_list.extend(default_mono.iter().cloned());
        } else {
            family_list.extend(default_prop.iter().cloned());
        }

        fonts.families.insert(
            egui::FontFamily::Name(family_key.clone().into()),
            family_list,
        );
        registered.insert(family_key);
    }

    for (idx, imp) in imported_fonts.iter().enumerate() {
        if let Ok(bytes) = fs::read(&imp.path) {
            let data_key = format!("imported_ttf_{idx}");
            let family_key = format!("custom:{}", imp.name);
            fonts.font_data.insert(
                data_key.clone(),
                Arc::new(egui::FontData::from_owned(bytes)),
            );

            let mut family_list = vec![data_key];
            family_list.extend(default_prop.iter().cloned());
            fonts.families.insert(
                egui::FontFamily::Name(family_key.clone().into()),
                family_list,
            );
            registered.insert(family_key);
        }
    }

    ctx.set_fonts(fonts);
    registered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_choice_toml_backwards_compatibility() {
        #[derive(Serialize, Deserialize)]
        struct Wrapper {
            font_family: FontFamilyChoice,
        }

        let old_toml = "font_family = \"Serif\"\n";
        let decoded: Wrapper = toml::from_str(old_toml).expect("decode legacy Serif");
        assert_eq!(decoded.font_family, FontFamilyChoice::Serif);

        let helv = Wrapper {
            font_family: FontFamilyChoice::Helvetica,
        };
        let s = toml::to_string(&helv).expect("serialize Helvetica");
        let rt: Wrapper = toml::from_str(&s).expect("roundtrip Helvetica");
        assert_eq!(rt.font_family, FontFamilyChoice::Helvetica);

        let custom = Wrapper {
            font_family: FontFamilyChoice::Custom("MyImportedFont".to_string()),
        };
        let cs = toml::to_string(&custom).expect("serialize Custom font");
        let crt: Wrapper = toml::from_str(&cs).expect("roundtrip Custom font");
        assert_eq!(
            crt.font_family,
            FontFamilyChoice::Custom("MyImportedFont".to_string())
        );
    }

    #[test]
    fn test_parse_ttf_family_name_rejects_invalid_bytes() {
        assert!(parse_ttf_family_name(b"not a ttf").is_err());
        assert!(parse_ttf_family_name(b"short").is_err());
    }

    #[test]
    fn test_import_real_windows_ttf_and_register_in_egui() {
        let sample_src = if Path::new(r"C:\Windows\Fonts\georgia.ttf").exists() {
            PathBuf::from(r"C:\Windows\Fonts\georgia.ttf")
        } else {
            return;
        };

        let temp_dir = std::env::temp_dir().join("test_import_ttf_font_dir");
        let _ = fs::remove_dir_all(&temp_dir);

        // Copy to a uniquely named .ttf to test import flow
        let staging_dir = temp_dir.join("staging");
        fs::create_dir_all(&staging_dir).unwrap();
        let staged_ttf = staging_dir.join("MyCustomLiterary.ttf");
        fs::copy(&sample_src, &staged_ttf).unwrap();

        let storage_dir = temp_dir.join("imported_fonts");
        let imported = import_ttf_font(&staged_ttf, &storage_dir, &[]).expect("import valid ttf");
        assert!(imported.path.exists());
        assert_eq!(imported.typst_family, "Georgia");
        assert_eq!(imported.name, "Georgia (2)");

        let ctx = egui::Context::default();
        let registered = register_egui_fonts(&ctx, &[imported.clone()]);
        let custom_choice = FontFamilyChoice::Custom(imported.name.clone());
        let expected_key = custom_choice.egui_family_key().unwrap();
        assert!(registered.contains(&expected_key));
        assert_eq!(
            custom_choice.to_egui_family(&registered),
            egui::FontFamily::Name(expected_key.into())
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_helvetica_is_in_builtin_choices() {
        let builtins = FontFamilyChoice::builtin_choices();
        assert!(builtins.contains(&FontFamilyChoice::Helvetica));
        assert_eq!(FontFamilyChoice::Helvetica.name(), "Helvetica");
        assert_eq!(
            FontFamilyChoice::Helvetica.typst_fallbacks(&[]),
            vec!["Helvetica".to_string(), "Arial".to_string()]
        );
    }
}
