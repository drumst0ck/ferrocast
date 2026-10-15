mod app;
mod clipboard_policy;
mod instance;
mod items;
mod services;
mod ui;

fn main() -> eframe::Result {
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
    eframe::run_native("Ferrocast", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, hidden)))))
}
