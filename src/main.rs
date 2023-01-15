use eframe::{egui::Visuals, run_native, NativeOptions};
use pipewire_graph_egui::app::NodeGraphExample;

fn main() {
    tracing_subscriber::fmt::init();

    run_native(
        env!("CARGO_PKG_NAME"),
        NativeOptions::default(),
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(Visuals::dark());
            Box::new(NodeGraphExample::new(cc))
        }),
    );
}
