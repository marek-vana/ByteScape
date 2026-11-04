mod app;
mod data;
mod gpu;
mod hud;
mod layout;
mod model;
mod platform;
mod scan;
mod ui;
mod validation;
fn main() -> eframe::Result {
    let native_args: Vec<_> = std::env::args_os().collect();
    if native_args.get(1).is_some_and(|a| a == "--scan-benchmark") {
        if let Err(error) = validation::scan_benchmark(&native_args) {
            eprintln!("Validation failed: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--benchmark") {
        app::benchmark();
        return Ok(());
    }
    let compact = args.iter().any(|a| a == "--compact");
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        multisampling: 4,
        wgpu_options: eframe::WgpuConfiguration {
            surface: eframe::egui_wgpu::SurfaceConfig::HIGH_THROUGHPUT,
            ..Default::default()
        },
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("ByteScape · Disk cartography")
            .with_inner_size(if compact {
                [1000.0, 700.0]
            } else {
                [1440.0, 900.0]
            })
            .with_min_inner_size([1000.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ByteScape",
        options,
        Box::new(|cc| Ok(Box::new(app::ByteScape::new(cc)))),
    )
}
