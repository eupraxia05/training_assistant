use bevy::prelude::*;
use bevy_cobweb_ui::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes_override: Some(true),
            ..Default::default()
        }))
        .add_plugins(CobwebUiPlugin)
        .load("main.cob")
        .add_systems(OnEnter(LoadState::Done), build_ui)
        .run();
}

fn build_ui(mut commands: Commands, mut s: SceneBuilder) {
    commands.spawn(Camera2d);
    commands.ui_root().spawn_scene_simple(("main.cob", "main_scene"), &mut s);
}
