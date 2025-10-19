use clap::{Command, ArgMatches};

pub struct App {
    plugins: Vec<Box<dyn Plugin>>,
    commands: Vec<(Command, fn(&ArgMatches) -> ())>
}

impl App {
    pub fn add_command(&mut self, command: Command, process_command_fn: fn(&ArgMatches) -> ()) {
        self.commands.push((command, process_command_fn));
    }

    pub fn new() -> Self {
        Self {
            plugins: Vec::default(),
            commands: Vec::default()
        }
    }

    pub fn add_plugin<P>(mut self, plugin: P) -> Self
        where P: Plugin + Clone + 'static
    {
        self.plugins.push(Box::new(plugin.clone()));
        plugin.build(&mut self);

        self
    }

    pub fn commands(&self) -> &Vec<(Command, fn(&ArgMatches) -> ())> {
        &self.commands
    }
}

pub trait Plugin {
    fn build(self, app: &mut App) -> ();
}

#[cfg(test)]
mod test {
    use crate::*;

    #[derive(Default, Clone)]
    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn build(self, app: &mut App) {
            app.add_command(Command::new("test"));
        }
    }

    #[test]
    fn add_command_test() {
        let mut app = App::new();
        app.add_plugin::<TestPlugin>(TestPlugin::default());
    }
}
