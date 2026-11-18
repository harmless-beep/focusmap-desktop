//! FocusMap desktop entry point.
//!
//! `--demo` seeds a temporary database with 45 days of realistic data and never
//! writes the real settings. `--minimized` starts in the tray.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ambient;
mod app;
mod autostart;
mod demo;
mod notify;
mod screens;
mod theme;
mod tray;
mod widgets;

use std::sync::Arc;

use focusmap_core::engine::Engine;
use focusmap_core::settings::Settings;
use focusmap_core::storage::Db;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let demo = args.iter().any(|arg| arg == "--demo");
    let minimized_arg = args.iter().any(|arg| arg == "--minimized");

    let settings = if demo {
        demo::settings()
    } else {
        Settings::load()
    };
    let db = match Db::open(&settings.db_path()) {
        Ok(db) => Arc::new(db),
        Err(err) => {
            eprintln!(
                "FocusMap could not open {}: {err}",
                settings.db_path().display()
            );
            return Ok(());
        }
    };
    if demo {
        demo::seed_if_needed(&db, &settings);
    }
    let engine = match Engine::start(&settings, db.clone()) {
        Ok(engine) => engine,
        Err(err) => {
            eprintln!("FocusMap could not start tracking: {err}");
            return Ok(());
        }
    };
    if settings.start_on_boot && !autostart::is_enabled() {
        let _ = autostart::set_enabled(true);
    }

    let start_hidden = minimized_arg || settings.start_minimized;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 880.0])
            .with_min_inner_size([1024.0, 700.0])
            .with_title("FocusMap")
            .with_icon(tray::window_icon())
            .with_visible(!start_hidden),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "FocusMap",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::FocusMapApp::new(
                cc, engine, db, settings, demo,
            )))
        }),
    )
}
