#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod app;
pub mod consts;
pub mod discord;
pub mod images;
pub mod models;
pub mod package;
pub mod search;
pub mod settings;
pub mod ui;

fn main() {
    // Same starting window as the original tool
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 760.0])
        .with_min_inner_size([860.0, 560.0])
        .with_title(app::APP_NAME);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let result = eframe::run_native(
        app::APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    );

    if let Err(e) = result {
        // Nothing else we can do out here, so just tell the user
        eprintln!("Data Package Tool failed to start: {e}");
        std::process::exit(1);
    }
}
