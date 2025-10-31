use clap::{Command, ArgMatches};
use std::result;
use std::ffi::OsString;

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

    pub fn add_plugin<P>(&mut self, plugin: P) -> &mut Self
        where P: Plugin + Clone + 'static
    {
        self.plugins.push(Box::new(plugin.clone()));
        plugin.build(self);

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

    pub fn execute(&self, command_str: &str) -> Result<CommandResponse> {
        let mut command = Command::new("tacl")
            .version("0.1.0")
            .about("Command line interface for Training Assistant")
            .subcommand_required(true);

        for (c, _) in self.commands() {
            command = command.subcommand(c);
        }

        let mut cmd_string = shlex::split(format!("tacl {}", command_str).as_str()).unwrap().iter().map(|e| OsString::from(e)).collect::<Vec<_>>();

        let matches = command.get_matches_from(cmd_string);

        let mut database_connection = self.open_db_connection()?;

        if let Some(subcommand_name) = matches.subcommand_name() {
            for (c, f) in self.commands() {
                if c.get_name() == subcommand_name {
                    if let Some(subcommand_matches) = matches.subcommand_matches(subcommand_name) {
                        let response = f(subcommand_matches, &mut database_connection)?;
                        return Ok(response);
                    }
                }
            }
        }
        Err(Error::UnknownError)
    }
}

#[derive(Default)]
pub struct CommandResponse {
    text: Option<String>
}

pub type ProcessCommandFn = fn(&ArgMatches, &mut DatabaseConnection) -> Result<CommandResponse>;

pub trait Plugin {
    fn build(self, app: &mut App) -> ();
}

#[derive(Debug)]
pub enum Error {
   FileError(String),
   DatabaseError(String),
   NoConnectionError,
   UnknownError
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
            Error,
            Result,
            CommandResponse,
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

    fn process_test_command(_arg_matches: &ArgMatches, _db_connection: &mut DatabaseConnection) -> Result<CommandResponse> {
        Ok(CommandResponse::default())
    }

    #[test]
    fn command_test() {
        let mut app = App::new();
        app.add_plugin::<TestPlugin>(TestPlugin::default());
        app.execute("test");
    }
}

