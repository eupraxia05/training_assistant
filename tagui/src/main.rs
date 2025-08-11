use egui_dock::{DockArea, DockState, TabViewer};
use std::path::PathBuf;

use handoutgen::HandoutConfig;
use rfd::FileDialog;

fn main() {
  let native_options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
      .with_inner_size([400.0, 300.0])
      .with_min_inner_size([300.0, 220.0])
      .with_icon(
        // NOTE: Adding an icon is optional
        eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
          .expect("Failed to load icon"),
      ),
      ..Default::default()
    };
    eframe::run_native(
        "eframe template",
        native_options,
        Box::new(|cc| Ok(Box::new(TrainingAssistantApp::default()))),
    ).expect("Failed to run app");
}

struct TrainingAssistantApp {
  dock_state: DockState<EditorTab>
}

impl Default for TrainingAssistantApp {
  fn default() -> Self {
    Self {
      dock_state: DockState::new(Vec::new())
    }
  }
}

impl eframe::App for TrainingAssistantApp {
  fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
      // The top panel is often a good place for a menu bar:
      egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
          if ui.button("New Handout...").clicked() {
            self.dock_state.main_surface_mut().push_to_first_leaf(EditorTab::new());
          }
          if ui.button("Save All").clicked() {
            for tab in self.dock_state.iter_all_tabs_mut() {
              tab.1.needs_save = false;
            }
          }
          if ui.button("Quit").clicked() {
              ctx.send_viewport_cmd(egui::ViewportCommand::Close);
          }
        });
      });
    });
    egui::CentralPanel::default().show(ctx, |ui| {
      DockArea::new(&mut self.dock_state).show(ctx, &mut EditorTabViewer::default())
    });
  }
}

struct EditorTab {
  handout_config: HandoutConfig,
  needs_save: bool,
  filepath: Option<PathBuf>
}

impl EditorTab {
  fn new() -> Self {
    Self {
      handout_config: Default::default(),
      needs_save: true,
      filepath: None
    }
  }

  fn save(&mut self) {
    if !self.needs_save {
      return;
    }

    if self.filepath.is_none() {
      self.filepath = FileDialog::new().save_file();
    }

    if self.filepath.is_none() {
      return;
    }
  }
}

#[derive(Default)]
struct EditorTabViewer;

impl TabViewer for EditorTabViewer {
  type Tab = EditorTab;

  fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
    if tab.needs_save {
      "tab*".into()
    } else {
      "tab".into()
    }
  }
  
  fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
    ui.horizontal(|ui| { 
      ui.label("Client name");
      ui.text_edit_singleline(&mut tab.handout_config.client_name);
    });
    ui.horizontal(|ui| { 
      ui.label("Date");
      ui.text_edit_singleline(&mut tab.handout_config.date);
    });
    ui.horizontal(|ui| { 
      ui.label("Summary");
      ui.text_edit_multiline(&mut tab.handout_config.summary);
    });
  }
}