//! The GUI application for Training Assistant.

use framework::prelude::*;
use gui::prelude::*;
use db_commands::DbCommandsPlugin;
use billing::InvoicePlugin;

fn main() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 240.0]),
        ..Default::default()
    };

    let mut context = Context::new();
    context.add_plugin(DbPlugin)?;
    context.add_plugin(DbCommandsPlugin)?;
    context.add_plugin(GuiPlugin)?;
    context.add_plugin(InvoicePlugin)?;

    context.startup()?;

    eframe::run_simple_native(
        "Training Assistant",
        options,
        move |ctx, _frame| {
            build_ui(&mut context, ctx);

            egui::CentralPanel::default().show(
                ctx,
                |ui| {

                    ui.label("hello world!");
                },
            );
        },
    ).expect("failed to run eframe app");

    Ok(())
}

fn build_ui(context: &mut Context, egui_ctx: &egui::Context) {
    gui::menu_ui(context, egui_ctx);
    egui::CentralPanel::default().show(egui_ctx, |ui| {
        ui.label("content");
    });
}
