mod app;
mod clipboard_policy;
mod instance;
mod items;
#[cfg(test)]
mod matrix_test;
mod media_tools;
#[cfg(test)]
mod media_tools_tests;
mod notes_ctl;
mod perf;
mod route;
mod secrets;
mod services;
mod ui;

/// Process start, for the FERROCAST_PROFILE first-frame measurement.
pub static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn main() -> eframe::Result {
    // First thing, before AppKit finishes launching: a ferrocast:// URL that launches the app is
    // delivered during that step and is buffered until the Notes callbacks are installed.
    ferrocast_macos::url_scheme::init();
    let _ = STARTED.set(std::time::Instant::now());
    let prof = std::env::var_os("FERROCAST_PROFILE").is_some();
    let data_dir = ferrocast_core::storage::Store::default_location().root;
    // Two copies would fight over the global hotkey and the clipboard history.
    let Some(_instance_lock) = instance::acquire(&data_dir) else {
        eprintln!("Ferrocast is already running.");
        return Ok(());
    };
    instance::install_panic_hook(data_dir);
    let hidden = std::env::args().any(|a| a == "--hidden");
    let viewport = egui::ViewportBuilder::default()
        .with_title("Ferrocast")
        .with_inner_size([ui::WIDTH, ui::HEIGHT])
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_resizable(false)
        .with_has_shadow(true)
        .with_taskbar(false)
        .with_visible(!hidden);
    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        event_loop_builder: Some(Box::new(|b| {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            b.with_activation_policy(ActivationPolicy::Accessory);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "Ferrocast",
        options,
        Box::new(move |cc| {
            if prof {
                eprintln!("window+gl ready after {:?}", STARTED.get().map(|t| t.elapsed()).unwrap_or_default());
            }
            let t = std::time::Instant::now();
            let app = app::App::new(cc, hidden);
            if prof {
                eprintln!("App::new {:?}", t.elapsed());
            }
            Ok(Box::new(app))
        }),
    )
}
