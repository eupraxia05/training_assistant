use bevy::prelude::*;
use bevy::ui::UiDebugOptions;
use bevy_cobweb::prelude::*;
use bevy_cobweb_ui::prelude::*;
use bevy_ui_text_input::{TextInputNode, TextInputMode, TextInputPrompt, TextInputPlugin};
use bevy::window::WindowResolution;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    watch_for_changes_override: Some(true),
                    ..Default::default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        window_theme: Some(bevy::window::WindowTheme::Dark),
                        resolution: WindowResolution::new(1280.0, 800.0)
                            .with_scale_factor_override(1.0),
                        ..default()
                    }),
                    ..default()
                })
        )
        /*.insert_resource(UiDebugOptions{
            enabled: true,
            show_hidden: true,
            show_clipped: true,
            ..default()
        })*/
        .insert_resource(UiScale(1.0)) 
        .add_plugins(CobwebUiPlugin)
        .add_plugins(TextInputPlugin)
        .load("main.cobweb")
        .add_systems(OnEnter(LoadState::Done), build_ui)
        .add_event::<AddProfileButtonPressedEvent>()
        .add_event::<NewProfileSubmitButtonPressedEvent>()
        .register_component_type::<SingleLineTextInput>()
        .add_systems(Update, on_single_line_text_input_added)
        .add_systems(Update, on_new_profile_submit_button_pressed)
        .run();
}

#[derive(Component, Reflect, PartialEq, Default)]
struct SingleLineTextInput;

#[derive(ReactComponent, Reflect, Default)]
struct SingleLineTextInputText(String);

fn on_single_line_text_input_added(mut commands: Commands, query: Query<Entity, Added<SingleLineTextInput>>) {
    for input in query.iter() {
        commands.ui_builder(input).insert(
            (
                TextInputNode {
                    mode: TextInputMode::SingleLine,
                    max_chars: Some(20),
                    clear_on_submit: false,
                    ..default()
                },
                TextInputPrompt::default(),
                React::<SingleLineTextInputText>::default()
            )
        );
    }
}

fn build_ui(mut commands: Commands, mut s: SceneBuilder) {
    let mut db_conn = training::DatabaseConnection::open_default().expect("couldn't open database");
    let clients = db_conn.clients().expect("couldn't get clients");

    commands.spawn(Camera2d);
    commands.ui_root().spawn_scene(("main.cobweb", "main_scene"), &mut s, |scene_handle| {
        let scene_root = scene_handle.id();
        scene_handle.despawn_on_event::<AddProfileButtonPressedEvent>();
        
        for client in clients {
            scene_handle.get("profiles_root::profile_buttons_root")
                .spawn_scene(("main.cobweb", "profile_button"), |scene_handle| {
                    scene_handle.get("text").update_text(client.name());
                });
        }
        scene_handle.get("profiles_root::add_root")
            .on_pressed(move |mut c: Commands, mut s: SceneBuilder| {
                c.entity(scene_root).despawn();
                c.ui_root().spawn_scene(("main.cobweb", "new_profile_form"), &mut s, |scene_handle| {
                    let new_profile_form_id = scene_handle.id();
                    scene_handle.get("content::submit").on_pressed(move |mut c: Commands, mut ew: EventWriter<NewProfileSubmitButtonPressedEvent>| {
                        scene_handle.get("content::name::input").
                        c.entity(new_profile_form_id).despawn();
                    });
                });
            });
    });
}

#[derive(Event, Default)]
struct AddProfileButtonPressedEvent;

#[derive(Event, Default)]
struct NewProfileSubmitButtonPressedEvent;

fn on_new_profile_submit_button_pressed() {

}
