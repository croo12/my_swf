#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod canvas;
mod repository_graph;
mod routing;
mod storage;
mod transport;

fn main() -> eframe::Result {
    let repository = std::env::args().nth(1).map(std::path::PathBuf::from);
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 940.0])
        .with_min_inner_size([960.0, 640.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!(
        "../../client/src-tauri/icons/128x128.png"
    )) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Planning Workbench",
        options,
        Box::new(move |cc| Ok(Box::new(app::Workbench::new(cc, repository)))),
    )
}
