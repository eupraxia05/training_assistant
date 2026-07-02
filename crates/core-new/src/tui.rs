use std::{default, fmt::Debug, rc::Rc, sync::Arc};

use crate::{CommandRunContext, CommandRunner};

use clap::{ArgMatches, Command};
use cursive::{
    Cursive, CursiveRunner,
    backend::Backend,
    event::{Event, Key},
    menu::Tree,
    view::{Nameable, Resizable, ViewWrapper},
    views::{
        BoxedView, Dialog, DummyView, LinearLayout,
        TextView, ViewRef,
    },
};
use cursive_tabs::{
    Align, Placement, TabPanel, TabView,
};

pub struct Tui {
    session_requested: bool,
    quit_requested: bool,
    cursive: Option<Cursive>,
}

#[derive(Default)]
struct TuiState {
    edit_state: EditState,
    tool_view_active: bool,
}

#[derive(Default)]
enum EditState {
    #[default]
    Empty,
    Schedule,
    Billing,
    Clients,
    Exercises,
    Console,
}

impl Tui {
    pub fn request_session(&mut self) {
        self.session_requested = true;
    }

    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    pub fn session_requested(&self) -> bool {
        self.session_requested
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn open_session(&mut self) {
        let mut cursive = Cursive::new();

        cursive.set_user_data(TuiState::default());

        cursive
            .menubar()
            .add_subtree(
                "Tools",
                Tree::new()
                    .leaf("Schedule", |s| {
                        Self::open_tool(
                            s,
                            EditState::Schedule,
                        )
                    })
                    .leaf("Billing", |s| {
                        Self::open_tool(
                            s,
                            EditState::Billing,
                        )
                    })
                    .leaf("Clients", |s| {
                        Self::open_tool(
                            s,
                            EditState::Clients,
                        )
                    })
                    .leaf("Exercises", |s| {
                        Self::open_tool(
                            s,
                            EditState::Exercises,
                        )
                    })
                    .leaf("Console", |s| {
                        Self::open_tool(
                            s,
                            EditState::Console,
                        )
                    }),
            )
            .add_subtree(
                "Config",
                Tree::new().leaf("Settings", |s| {}),
            )
            .add_subtree(
                "Help",
                Tree::new().leaf(
                    "About Training Assistant",
                    |s| {},
                ),
            )
            .add_delimiter()
            .add_leaf("Quit", |s| s.quit());

        cursive.add_global_callback(
            Event::Char(' '),
            |s| s.select_menubar(),
        );

        /*let mut tabs = TabPanel::new()
            .with_bar_alignment(Align::Start)
            .with_bar_placement(
                Placement::HorizontalBottom,
            );

        tabs.add_tab(
            TextView::new("tab 1 content")
                .with_name("Tab 1"),
        );
        tabs.add_tab(
            TextView::new("tab 2 content")
                .with_name("Tab 2"),
        );

        let layout = LinearLayout::vertical()
            .child(DummyView.fixed_height(1))
            /*.child(Dialog::around(
                tabs.with_name("MainTabPanel"),
            ))*/;

        cursive.add_layer(layout);*/

        /*cursive.add_global_callback(Key::Left, |s| {
            let mut tabs: ViewRef<TabPanel> =
                s.find_name("MainTabPanel").unwrap();
            tabs.prev();
        });
        cursive.add_global_callback(Key::Right, |s| {
            let mut tabs: ViewRef<TabPanel> =
                s.find_name("MainTabPanel").unwrap();
            tabs.next();
        });*/

        /*cursive.add_layer(
            Dialog::around(TextView::new(
                "hello world",
            ))
            .title("Training Assistant")
            .button("Quit", |s| s.quit()),
        );*/

        self.cursive = Some(cursive);
    }

    pub fn run_session<F, E>(
        &mut self,
        backend_init: F,
    ) where
        F: FnOnce() -> std::result::Result<
            Box<dyn Backend>,
            E,
        >,
        E: Debug,
    {
        self.open_session();
        if let Some(cursive) = &mut self.cursive {
            let run_result =
                cursive.try_run_with(backend_init);
            if let Err(e) = run_result {
                println!("{:?}", e)
            }
        }
    }

    pub fn runner<B>(
        &mut self,
        backend: Box<B>,
    ) -> Option<CursiveRunner<&mut Cursive>>
    where
        B: Backend + 'static,
    {
        if let Some(cursive) = &mut self.cursive {
            Some(cursive.runner(backend))
        } else {
            None
        }
    }

    fn refresh_tool_view(cursive: &mut Cursive) {
        if cursive
            .user_data::<TuiState>()
            .unwrap()
            .tool_view_active
        {
            cursive.pop_layer();
        }

        match cursive
            .user_data::<TuiState>()
            .unwrap()
            .edit_state
        {
            EditState::Empty => {}
            EditState::Billing => {
                cursive.add_layer(
                    Dialog::new().title("Billing"),
                );
                cursive
                    .user_data::<TuiState>()
                    .unwrap()
                    .tool_view_active = true;
            }
            EditState::Schedule => {
                cursive.add_layer(
                    Dialog::new().title("Schedule"),
                );
                cursive
                    .user_data::<TuiState>()
                    .unwrap()
                    .tool_view_active = true;
            }
            EditState::Clients => {
                cursive.add_layer(
                    Dialog::new().title("Clients"),
                );
                cursive
                    .user_data::<TuiState>()
                    .unwrap()
                    .tool_view_active = true;
            }
            EditState::Exercises => {
                cursive.add_layer(
                    Dialog::new().title("Exercises"),
                );
                cursive
                    .user_data::<TuiState>()
                    .unwrap()
                    .tool_view_active = true;
            }
            EditState::Console => {
                cursive.add_layer(
                    Dialog::new().title("Console"),
                );
                cursive
                    .user_data::<TuiState>()
                    .unwrap()
                    .tool_view_active = true;
            }
        };
    }

    fn open_tool(
        cursive: &mut Cursive,
        edit_state: EditState,
    ) {
        cursive
            .user_data::<TuiState>()
            .unwrap()
            .edit_state = edit_state;
        Self::refresh_tool_view(cursive);
    }
}

impl Default for Tui {
    fn default() -> Self {
        Self {
            session_requested: false,
            quit_requested: false,
            cursive: None,
        }
    }
}

pub(crate) fn add_builtin_commands(
    command_runner: &mut CommandRunner,
) {
    command_runner.add_command(
        Command::new("tui"),
        run_tui_command_fn,
    );
}

fn run_tui_command_fn(
    _: &ArgMatches,
    mut context: CommandRunContext,
) -> String {
    if context.tui.is_none() {
        return "TUI not provided by command runner."
            .into();
    }

    context.tui.as_mut().unwrap().request_session();
    return "Done.".into();
}

#[cfg(test)]
mod tests {
    use cursive::backends::{
        crossterm::Backend,
        puppet::Backend as PuppetBackend,
    };

    use crate::{
        CommandRunContext, CommandRunner, tui::Tui,
    };

    #[test]
    fn tui_command() {
        let command_runner =
            CommandRunner::with_builtin_commands();

        let mut tui = Tui::default();

        let context = CommandRunContext {
            tui: Some(&mut tui),
        };

        assert_eq!(
            command_runner.run(
                "tacl",
                &["tacl", "tui"],
                context,
            ),
            "Done."
        );

        assert!(tui.session_requested);
    }

    #[test]
    fn step_with_runner() {
        let mut tui = Tui::default();

        tui.open_session();
        assert!(tui.cursive.is_some());

        {
            let backend = PuppetBackend::init(Some(
                cursive::XY { x: 36, y: 16 },
            ));

            let stream = backend.stream();
            backend
                .input()
                .send(Some(
                    cursive::event::Event::Refresh,
                ))
                .unwrap();

            insta::assert_snapshot!(
                stream.try_recv().unwrap()
            );

            let mut runner =
                tui.runner(backend).unwrap();
            runner.step();
            runner;

            insta::assert_snapshot!(
                stream.try_recv().unwrap()
            );
        }

        {
            let backend = PuppetBackend::init(Some(
                cursive::XY { x: 36, y: 16 },
            ));

            let stream = backend.stream();
            backend
                .input()
                .send(Some(
                    cursive::event::Event::Key(
                        cursive::event::Key::Enter,
                    ),
                ))
                .unwrap();

            let mut runner =
                tui.runner(backend).unwrap();
            runner.step();

            assert!(!runner.is_running());
        }
    }
}
