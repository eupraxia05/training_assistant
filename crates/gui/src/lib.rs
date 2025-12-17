//! A GUI frontend implementation for Training Assistant.
use framework::prelude::*;
use std::any::Any;

#[derive(Clone)]
pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(self, context: &mut Context) -> Result<()> {
        if !context.has_resource::<GuiNewWindowTypes>() {
            context.add_resource(GuiNewWindowTypes::default());
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct GuiNewWindowTypes {
    types: Vec<GuiNewWindowType>
}

pub struct GuiNewWindowType {
    name: String
}

pub trait GuiContextExt {
    fn add_new_window_type<T>(&mut self, name: impl Into<String>);
}

impl GuiContextExt for Context {
    fn add_new_window_type<T>(&mut self, name: impl Into<String>) {
        if !self.has_resource::<GuiNewWindowTypes>() {
            self.add_resource(GuiNewWindowTypes::default());
        }

        if let Some(types) = self.get_resource_mut::<GuiNewWindowTypes>() {
            types.types.push(GuiNewWindowType {
                name: name.into()
            });
        }
    }
}

impl Resource for GuiNewWindowTypes {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub fn menu_ui(context: &mut Context, egui_ctx: &egui::Context) {
    egui::TopBottomPanel::top("menu_panel").show(egui_ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                ui.button("Quit");
            });
            ui.menu_button("Window", |ui| {
                if let Some(new_window_types) = context.get_resource::<GuiNewWindowTypes>() {
                    for t in &new_window_types.types {
                        ui.button(t.name.clone());    
                    }
                }
            });
            ui.menu_button("Help", |ui| {
                ui.button("About Training Assistant...");
            });
        });
    });
}

struct AboutWindow;

pub mod prelude {
    pub use crate::{
        GuiPlugin,
        GuiNewWindowTypes,
        GuiContextExt
    };
}
