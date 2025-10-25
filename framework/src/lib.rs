use clap::{Command, ArgMatches};
use std::result;

mod db;
use db::DatabaseConnection;
use db::TableConfig;
use db::RowType;

pub struct App {
    plugins: Vec<Box<dyn Plugin>>,
    commands: Vec<(Command, ProcessCommandFn)>,
    tables: Vec<TableConfig>,
}

impl App {
    pub fn add_command(&mut self, command: Command, process_command_fn: ProcessCommandFn) -> &mut Self {
        self.commands.push((command, process_command_fn));
        self
    }

    pub fn new() -> Self {
        Self {
            plugins: Vec::default(),
            commands: Vec::default(),
            tables: Vec::default(),
        }
    }

    pub fn add_plugin<P>(mut self, plugin: P) -> Self
        where P: Plugin + Clone + 'static
    {
        self.plugins.push(Box::new(plugin.clone()));
        plugin.build(&mut self);

        self
    }

    pub fn commands(&self) -> &Vec<(Command, ProcessCommandFn)> {
        &self.commands
    }

    pub fn add_table<R>(&mut self, table_name: String) -> &mut Self
        where R: RowType
    {
        self.tables.push(TableConfig {
            table_name,
            setup_fn: R::setup
        });

        self 
    }

    pub fn open_db_connection(&self) -> Result<DatabaseConnection> {
        DatabaseConnection::open_default(&self.tables)
    }
}

pub type ProcessCommandFn = fn(&ArgMatches, &mut DatabaseConnection) -> Result<()>;

pub trait Plugin {
    fn build(self, app: &mut App) -> ();
}

#[derive(Debug)]
pub enum Error {
   FileError(String),
   DatabaseError(String),
   NoConnectionError
}

pub type Result<T, E = Error> = result::Result<T, E>; 

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::DatabaseError(e.to_string())
    }
}

pub mod prelude {
    pub use {
        crate::{
            App,
            Plugin,
            db::{
                DbPlugin,
                DatabaseConnection,
                RowId,
                RowType,
                FieldType,
                Trainer,
                Client,
                TableConfig
            }
        }
    };
}

#[cfg(test)]
mod test {
    use crate::*;

    #[derive(Default, Clone)]
    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn build(self, app: &mut App) {
            app.add_command(Command::new("test"), process_test_command);
        }
    }

    fn process_test_command(arg_matches: &ArgMatches, db_connection: &mut DatabaseConnection) -> Result<()> {
        Ok(())
    }

    #[test]
    fn add_command_test() {
        let mut app = App::new();
        app.add_plugin::<TestPlugin>(TestPlugin::default());
    }
}

