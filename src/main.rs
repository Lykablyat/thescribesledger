#![windows_subsystem = "windows"]

use std::sync::Arc;
use the_scribes_ledger::ScribesLedgerApp;

fn load_app_icon() -> Arc<eframe::egui::IconData> {
    let img_bytes = include_bytes!("../logo_asset/new_logo.png");
    if let Ok(dyn_img) = image::load_from_memory(img_bytes) {
        let rgba = dyn_img
            .resize_exact(128, 128, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let (width, height) = rgba.dimensions();
        Arc::new(eframe::egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    } else {
        Arc::new(eframe::egui::IconData::default())
    }
}

fn main() -> eframe::Result<()> {
    let icon = load_app_icon();
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1150.0, 750.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("The Scribe's Ledger v1.0.2")
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "The Scribe's Ledger v1.0.2",
        native_options,
        Box::new(|cc| Ok(Box::new(ScribesLedgerApp::new(cc)))),
    )
}
