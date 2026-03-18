mod action;
mod app;
mod config;
mod focus;
mod global_mouse;
mod search;

use app::QuickerApp;
use config::Config;

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let config = Config::load();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([config.panel_width, config.panel_height])
            .with_min_inner_size([300.0, 200.0])
            .with_title("Quicker-RS"),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Quicker-RS",
        options,
        Box::new(|cc| Ok(Box::new(QuickerApp::new(cc)))),
    )
}
