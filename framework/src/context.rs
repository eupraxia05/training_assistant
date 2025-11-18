use crate::db::{
    DbConnection, TableConfig, TableRow,
};
use crate::{Error, Result};

use clap::{ArgMatches, Command};
use std::ffi::OsString;

/// A loose application-layer framework shared across
/// command-line and GUI interfaces.
///
/// ```
/// # use framework::prelude::*;
/// # use clap::{Command, ArgMatches};
/// #[derive(Clone)]
/// struct MyPlugin;
///
/// impl Plugin for MyPlugin {
///     fn build(self, context: &mut Context) {
///         context.add_command(
///             Command::new("foo"),
///             process_foo_command
///         );
///     }
/// }
///
/// fn process_foo_command(
///     _: &ArgMatches,
///     _: &mut DbConnection
///     ) -> Result<CommandResponse>
/// {
///     Ok(CommandResponse::new("foo command invoked"))
/// }
///
/// // ...
///
/// # fn main() -> Result<()> {
/// let mut context = Context::new();
/// # context.in_memory_db(true);
/// context.add_plugin(MyPlugin);
/// context.startup()?;
/// let db_connection = context.db_connection()?;
/// # Ok(())
/// # }
/// ```
// TODO: the add-plugins -> setup -> get connection pattern
// seems error prone. Probably better to split the first
// step into a builder.
pub struct Context {
    /// The `Plugin`s registered with this `Context`.
    /// Add one with `Context::add_plugin`.
    plugins: Vec<Box<dyn Plugin>>,

    /// The `Command`s registered with this `Context`.
    /// Add one with `Context::add_command`.
    commands: Vec<(Command, ProcessCommandFn)>,

    /// The tables this Context requests when
    /// opening a database connection. Add a table
    /// with `Context::add_table`.
    tables: Vec<TableConfig>,

    /// The database connection this Context creates.
    /// Opened when `startup` is called.
    db_connection: Option<DbConnection>,

    /// Whether or not the db connection should be opened in memory.
    open_db_in_memory: bool,
}

impl Context {
    /// Creates a new Context, without any plugins,
    /// commands, or tables.
    pub fn new() -> Self {
        Self {
            plugins: Vec::default(),
            commands: Vec::default(),
            tables: Vec::default(),
            db_connection: None,
            open_db_in_memory: false,
        }
    }

    /// Registers a plugin with the context.
    pub fn add_plugin<P>(
        &mut self,
        plugin: P,
    ) -> &mut Self
    where
        P: Plugin + Clone + 'static,
    {
        self.plugins.push(Box::new(plugin.clone()));
        plugin.build(self);

        self
    }

    /// Registers a new command with the context.
    ///
    /// * `command`: The command to register.
    /// * `process_command_fn`: A callback function
    ///     called with the command and its args
    ///     when the command is invoked.
    pub fn add_command(
        &mut self,
        command: Command,
        process_command_fn: ProcessCommandFn,
    ) -> &mut Self {
        self.commands
            .push((command, process_command_fn));
        self
    }

    /// Registers a new table with the context. Must
    /// be called before `open_db_connection`.
    pub fn add_table<R>(
        &mut self,
        table_name: impl Into<String>,
    ) -> &mut Self
    where
        R: TableRow,
    {
        self.tables.push(TableConfig {
            table_name: table_name.into(),
            setup_fn: R::setup,
            push_tabled_header_fn: R::push_tabled_header,
            push_tabled_record_fn: R::push_tabled_record,
        });

        self
    }

    /// Sets whether or not the database should be
    /// opened in-memory. Useful for testing.
    /// Defaults to `false` and does not need to
    /// be called if a db file is desired.
    pub fn in_memory_db(&mut self, in_memory: bool) {
        self.open_db_in_memory = in_memory;
    }

    /// Opens a database connection. Uses the default
    /// db path, unless built in test config, in
    /// which case it uses a separate test db path.
    fn open_db_connection(
        &self,
    ) -> Result<DbConnection> {
        if self.open_db_in_memory {
            DbConnection::open_test(self.tables.clone())
        } else {
            DbConnection::open_default(
                self.tables.clone(),
            )
        }
    }

    /// Call this after adding plugins, tables, and 
    /// commands. Opens a connection to the database.
    pub fn startup(&mut self) -> Result<()> {
        self.db_connection = Some(self.open_db_connection()?);

        Ok(())
    }

    /// Gets the database connection. Must be called 
    /// after `setup`. Returns 
    /// `Err(Error::NoConnectionError)` if the
    /// connection has not been created yet.
    pub fn db_connection(&mut self) -> Result<&mut DbConnection> {
        if let Some(db_connection) = &mut self.db_connection {
            Ok(db_connection)
        } else {
            Err(Error::NoConnectionError)
        }
    }

    /// Executes a registered command from the given
    /// string. The string is parsed using `clap`,
    /// starting at the first argument.
    ///
    /// # Example
    ///
    /// ```
    /// # use framework::prelude::*;
    /// # fn main() -> Result<()> {
    /// # let mut context = Context::new();
    /// # context.add_plugin(DbPlugin);
    /// # context.in_memory_db(true);
    /// # context.startup()?;
    /// context.execute("new --table trainer")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn execute(
        &mut self,
        command_str: &str,
    ) -> Result<CommandResponse> {
        let Some(db_connection) = &mut self.db_connection else {
            return Err(Error::NoConnectionError);
        };

        // todo: this uses `tacl` as a dummy binary name
        // to make clap argument parsing work, but
        // it seems silly.
        let mut command = Command::new("tacl")
            .version("0.1.0")
            .about("Command line interface for Training Assistant")
            .subcommand_required(true);

        for (c, _) in &self.commands {
            command = command.subcommand(c);
        }

        let cmd_string = shlex::split(
            format!("tacl {}", command_str).as_str(),
        )
        .unwrap()
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>();

        let matches =
            command.get_matches_from(cmd_string);

        if let Some(subcommand_name) =
            matches.subcommand_name()
        {
            for (c, f) in &self.commands {
                if c.get_name() == subcommand_name
                    && let Some(subcommand_matches) =
                        matches.subcommand_matches(
                            subcommand_name,
                        )
                {
                    let response = f(
                        subcommand_matches,
                        db_connection,
                    )?;
                    return Ok(response);
                }
            }
        }
        Err(Error::UnknownError)
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// The result of running a successful command.
/// Optionally contains a text response to
/// be displayed.
#[derive(Default)]
pub struct CommandResponse {
    text: Option<String>,
    tui_requested: bool,
    tui_render_fn: Option<TuiRenderFn>,
    tui_update_fn: Option<TuiUpdateFn>
}

#[derive(Default)]
pub struct TuiState {
    should_quit: bool
}

impl TuiState {
    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn request_quit(&mut self) {
        self.should_quit = true;
    }
}

pub type TuiRenderFn = fn (&mut ratatui::Frame);

pub type TuiUpdateFn = fn (&mut TuiState, &crossterm::event::Event);

impl CommandResponse {
    /// Creates a new CommandResponse with a
    /// text response. Use `Default::default()`
    /// to create a response without text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            tui_requested: false,
            tui_render_fn: None,
            tui_update_fn: None
        }
    }

    pub fn request_tui(mut self, tui_render_fn: TuiRenderFn, tui_update_fn: TuiUpdateFn) -> Self {
        self.tui_requested = true;
        self.tui_render_fn = Some(tui_render_fn);
        self.tui_update_fn = Some(tui_update_fn);
        self
    }

    pub fn tui_requested(&self) -> bool {
        self.tui_requested
    }
    
    pub fn tui_render_fn(&self) -> Option<TuiRenderFn> {
        self.tui_render_fn
    }

    pub fn tui_update_fn(&self) -> Option<TuiUpdateFn> {
        self.tui_update_fn
    }

    /// Gets a copy of the text of the response, if it exists.
    pub fn text(&self) -> Option<String> {
        self.text.clone()
    }
}

type ProcessCommandFn = fn(
    &ArgMatches,
    &mut DbConnection,
) -> Result<CommandResponse>;

/// An interface for adding functionality to a Context. Inspired by Bevy's plugin interface.
pub trait Plugin {
    /// Runs on adding the plugin to a Context. Use this to register commands, add tables, etc.
    fn build(self, context: &mut Context) -> ();
}

#[cfg(test)]
mod test {
    use crate::prelude::*;
    use clap::{ArgMatches, Command};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default, Clone)]
    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn build(self, context: &mut Context) {
            context.add_command(
                Command::new("test"),
                process_test_command,
            );
            context.add_command(
                Command::new("test2"),
                process_test2_command,
            );
        }
    }

    static COMMAND_EXECUTED_COUNTER: AtomicUsize =
        AtomicUsize::new(0);

    fn process_test_command(
        _arg_matches: &ArgMatches,
        _db_connection: &mut DbConnection,
    ) -> Result<CommandResponse> {
        COMMAND_EXECUTED_COUNTER
            .store(1, Ordering::Relaxed);
        Ok(CommandResponse::default())
    }

    static COMMAND2_EXECUTED_COUNTER: AtomicUsize =
        AtomicUsize::new(0);

    fn process_test2_command(
        _arg_matches: &ArgMatches,
        _db_connection: &mut DbConnection,
    ) -> Result<CommandResponse> {
        COMMAND2_EXECUTED_COUNTER
            .store(1, Ordering::Relaxed);
        Ok(CommandResponse::new("foobar"))
    }

    #[test]
    fn plugin_with_commands_test() -> Result<()> {
        let mut context = Context::new();
        context.add_plugin::<TestPlugin>(
            TestPlugin::default(),
        );
        context.in_memory_db(true);
        assert_eq!(context.commands.len(), 2);
        context.startup()?;
        let response = context.execute("test")?;
        assert_eq!(
            COMMAND_EXECUTED_COUNTER
                .load(Ordering::Relaxed),
            1
        );
        assert!(response.text().is_none());
        let response2 = context.execute("test2")?;
        assert_eq!(
            COMMAND2_EXECUTED_COUNTER
                .load(Ordering::Relaxed),
            1
        );
        assert!(response2.text().is_some());
        assert_eq!(
            response2.text().clone().unwrap(),
            "foobar"
        );
        Ok(())
    }
}
