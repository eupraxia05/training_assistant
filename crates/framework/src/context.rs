use crate::db::{
    DbConnection,
};
use crate::{Error, Result};

use clap::{ArgMatches, Command};
use std::ffi::OsString;
use std::collections::HashMap;
use std::any::{Any, TypeId};

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
///     _: &mut Context,
///     _: &ArgMatches,
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
/// let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;
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

    /// Whether or not the db connection should be opened in memory.
    open_db_in_memory: bool,

    resources: HashMap<TypeId, Box<dyn Resource>>
}

impl Context {
    /// Creates a new Context, without any plugins,
    /// commands, or tables.
    pub fn new() -> Self {
        Self {
            plugins: Vec::default(),
            commands: Vec::default(),
            open_db_in_memory: false,
            resources: HashMap::new(),
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

    /// Sets whether or not the database should be
    /// opened in-memory. Useful for testing.
    /// Defaults to `false` and does not need to
    /// be called if a db file is desired.
    pub fn in_memory_db(&mut self, in_memory: bool) {
        self.open_db_in_memory = in_memory;
    }

    /// Adds a `Resource` to the resource registry. Overwrites
    /// the existing entry if it already exists.
    pub fn add_resource<R>(&mut self, res: R)
        where R: Resource
    {
        self.resources.insert(TypeId::of::<R>(), Box::new(res));
    }

    /// Gets a mutable `Resource` reference by type if it was 
    /// added with `add_resource`. Otherwise, returns `None`.
    pub fn get_resource_mut<R>(&mut self) -> Option<&mut R>
        where R: Resource
    {
        let boxed = self.resources.get_mut(&TypeId::of::<R>());
        if let Some(b) = boxed {
            if let Some(r) = b.as_any_mut().downcast_mut::<R>() {
                return Some(r);
            }
        }
        None
    }

    /// Gets a `Resource` reference by type if it was added
    /// with `add_resource`. Otherwise, returns `None`.
    pub fn get_resource<R>(&self) -> Option<&R>
        where R: Resource
    {
        let boxed = self.resources.get(&TypeId::of::<R>());
        if let Some(b) = boxed {
            if let Some(r) = b.as_any().downcast_ref::<R>() {
                return Some(r);
            }
        }
        None
    }

    /// Returns `true` if this `Context` has the specified `Resource` type.
    pub fn has_resource<R>(&self) -> bool 
        where R: Resource
    {
        self.resources.contains_key(&TypeId::of::<R>())
    }

    /// Call this after adding plugins, tables, and 
    /// commands. Opens a connection to the database.
    pub fn startup(&mut self) -> Result<()> {
        let db_connection = if self.open_db_in_memory {
            DbConnection::open_test(self)
        } else {
            DbConnection::open_default(self)
        }?;

        self.add_resource(db_connection);

        Ok(())
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
    /// let response = context.execute("--help")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn execute(
        &mut self,
        command_str: &str,
    ) -> Result<CommandResponse> {
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
                        self,
                        subcommand_matches,
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
}

impl CommandResponse {
    /// Creates a new CommandResponse with a
    /// text response. Use `Default::default()`
    /// to create a response without text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
        }
    }

    /// Gets a copy of the text of the response, if it exists.
    pub fn text(&self) -> Option<String> {
        self.text.clone()
    }
}

type ProcessCommandFn = fn(
    &mut Context,
    &ArgMatches,
) -> Result<CommandResponse>;

/// An interface for adding functionality to a Context. Inspired by Bevy's plugin interface.
pub trait Plugin {
    /// Runs on adding the plugin to a Context. Use this to register commands, add tables, etc.
    fn build(self, context: &mut Context) -> ();
}

/// A singleton data type managed by a `Context`. Use `Context::add_resource` to add one,
/// and `Context::get_resource<T>` to get it.
pub trait Resource: Any {
    /// Returns this type as a reference to a generic `std::any::Any` type.
    fn as_any(&self) -> &dyn Any;

    /// Retyrns this type as a mutable reference to a generic `std::any::Any` type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[cfg(test)]
mod test {
    use crate::prelude::*;
    use clap::{ArgMatches, Command};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::any::Any;

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
            context.add_resource(TestResource::default());
        }
    }

    #[derive(Default)]
    struct TestResource {
        foo: i32
    }

    impl Resource for TestResource {
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }
    }

    static COMMAND_EXECUTED_COUNTER: AtomicUsize =
        AtomicUsize::new(0);

    fn process_test_command(
        _context: &mut Context,
        _arg_matches: &ArgMatches,
    ) -> Result<CommandResponse> {
        COMMAND_EXECUTED_COUNTER
            .store(1, Ordering::Relaxed);
        Ok(CommandResponse::default())
    }

    static COMMAND2_EXECUTED_COUNTER: AtomicUsize =
        AtomicUsize::new(0);

    fn process_test2_command(
        _context: &mut Context,
        _arg_matches: &ArgMatches,
    ) -> Result<CommandResponse> {
        COMMAND2_EXECUTED_COUNTER
            .store(1, Ordering::Relaxed);
        Ok(CommandResponse::new("foobar"))
    }

    #[test]
    fn plugin_test() -> Result<()> {
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
        let res = context.get_resource_mut::<TestResource>();
        assert!(res.is_some());
        assert_eq!(res.as_ref().unwrap().foo, 0);
        res.unwrap().foo = 42;
        assert_eq!(context.get_resource_mut::<TestResource>().unwrap().foo, 42);
        Ok(())
    }
}
