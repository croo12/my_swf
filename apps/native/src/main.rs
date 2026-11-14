#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod canvas;
mod file_access;
mod repository_graph;

#[cfg(test)]
#[path = "../build_support/source_limits.rs"]
mod source_limits;

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--export-router-sources") {
        let Some(directory) = args.get(2) else {
            eprintln!("--export-router-sources <directory>");
            std::process::exit(2);
        };
        if let Err(error) = factory_layout::export_sources(directory.as_ref()) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("--check-router") {
        if let Err(error) = factory_layout::check_runtime() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("--compare-layout") {
        match (args.get(2), args.get(3)) {
            (Some(repository), Some(output)) => {
                if let Err(error) =
                    factory_layout_comparison::run(repository.as_ref(), output.as_ref(), |path| {
                        let [r, g, b, _] = canvas::layer_color(path).to_array();
                        [r, g, b]
                    })
                {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("--compare-layout <repository> <output-directory>");
                std::process::exit(2);
            }
        }
        return Ok(());
    }
    let repository = std::env::args().nth(1).map(std::path::PathBuf::from);
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 940.0])
        .with_min_inner_size([960.0, 640.0]);
    if let Ok(icon) =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/128x128.png"))
    {
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
