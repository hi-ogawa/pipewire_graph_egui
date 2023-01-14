use eframe::{egui::Visuals, run_native, NativeOptions};
use pipewire_graph_egui::app::NodeGraphExample;

fn main() {
    run_native(
        "pipewire graph egui",
        NativeOptions::default(),
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(Visuals::dark());
            Box::new(NodeGraphExample::new(cc))
        }),
    );
}