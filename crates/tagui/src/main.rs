//! The GUI application for Training Assistant.

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    eframe::run_simple_native(
        "Training Assistant",
        options,
        move |ctx, _frame| {
            egui::CentralPanel::default().show(
                ctx,
                |ui| {
                    ui.label("hello world!");
                },
            );
        },
    )
}
