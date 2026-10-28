mod app;
mod services;
mod models;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PLauncher 1.7.0")
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PLauncher",
        options,
        Box::new(|cc| Ok(Box::new(app::PLauncherApp::new(cc)))),
    )
}
