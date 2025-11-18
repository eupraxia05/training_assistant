use crate::{
    Error, Result,
    context::{CommandResponse, Context, Plugin, TuiState, Resource},
};
use clap::{Arg, ArgMatches, Command};
use framework_derive_macros::TableRow;
use rusqlite::{
    Connection, ToSql, params, types::FromSql,
};
use std::fs;
use std::path::PathBuf;
use std::fmt::{Display, Formatter};
use tabled::{builder::Builder as TabledBuilder};
use ratatui::{
    style::Stylize,
    text::Line,
    widgets::{Block, Paragraph}
};
use crossterm::event::{Event, KeyEvent, KeyCode, KeyEventKind};
use std::any::Any;

//////////////////////////////////////////////////////
// PUBLIC API
//////////////////////////////////////////////////////

/// Add this plugin to a `Context` to add default 
/// tables and basic database editing commands.
#[derive(Default, Clone)]
pub struct DbPlugin;

/// A connection to the underlying SQLite database.
pub struct DbConnection {
    // The rusqlite connection. None if it's closed
    // or not opened yet.
    connection: Option<Connection>,

    // The filepath the database was opened from.
    // None if it's an in-memory connection.
    db_path: Option<PathBuf>,

    // The tables this connection was created with.
    // TODO: this is duplicated between here and
    // Context
    tables: Vec<TableConfig>,
}

impl DbConnection {
    /// Returns true if the connection is open.
    pub fn is_open(&self) -> bool {
        self.connection.is_some()
    }

    /// Gets the path of the database file opened 
    /// by this connection.
    pub fn db_path(&self) -> &Option<PathBuf> {
        &self.db_path
    }

    /// Closes and deletes the database. Requires a 
    /// currently open connection, and will close it 
    /// before deleting the file. If this is an 
    /// in-memory database connection without a file, 
    /// it will just close the connection.
    ///
    /// Returns `Ok` if the database was successfully
    /// closed and deleted.
    pub fn delete_db(&mut self) -> Result<()> {
        // check if the connection exists
        // not using get_connection_if_exists because
        // we intend to consume the connection
        let Some(connection) = self.connection.take()
        else {
            return Err(Error::NoConnectionError);
        };

        // close the connection
        connection.close().map_err(|e| e.1)?;

        // erase the database file (if it exists)
        if let Some(db_path) = &self.db_path {
            std::fs::remove_file(db_path.clone())
                .map_err(|e| {
                    Error::FileError(e.to_string())
                })?;
            self.db_path = None;
        }

        Ok(())
    }

    /// Inserts a new empty row into a table, setting
    /// fields to default values.
    ///
    /// * `table` - The table name to insert a row into.
    pub fn new_row_in_table(
        &mut self,
        table: impl Into<String>,
    ) -> Result<RowId> {
        let connection = self.get_connection_if_exists()?;

        // execute the INSERT command with the given
        // table
        connection.execute(
            format!(
                "INSERT INTO {} DEFAULT VALUES;",
                table.into()
            )
            .as_str(),
            [],
        )?;

        // get the last inserted row ID
        // this is guaranteed to be valid and match
        // the intended row, as we would have already
        // exited if insertion failed
        let row_id = connection.last_insert_rowid();

        Ok(RowId(row_id))
    }

    /// Sets a field in a table to a given value.
    /// Returns `Ok` if the field was successfully
    /// set.
    ///
    /// * `table` - The table name.
    /// * `row_id` - The id of the row.
    /// * `field` - The field name.
    /// * `value` - The value to set (must
    ///   implement `ToSql`)
    pub fn set_field_in_table<V>(
        &mut self,
        table: impl Into<String>,
        row_id: RowId,
        field: impl Into<String>,
        value: V,
    ) -> Result<()>
    where
        V: ToSql,
    {
        let connection = self.get_connection_if_exists()?;

        connection.execute(
            format!(
                "UPDATE {} SET {} = ?1 WHERE id = ?2",
                table.into(),
                field.into()
            )
            .as_str(),
            params![value, row_id.0],
        )?;

        Ok(())
    }

    /// Gets all the available row IDs in a given table.
    ///
    /// * `table` - The name of the table to get row IDs from.
    pub fn get_table_row_ids(
        &self,
        table: impl Into<String>,
    ) -> Result<Vec<i64>> {
        let connection = self.get_connection_if_exists()?;

        let mut select = connection.prepare(
            format!("SELECT id FROM {}", table.into())
                .as_str(),
        )?;

        Ok(select
            .query_map([], |row| row.get(0))?
            .filter_map(|c| c.ok())
            .collect())
    }

    /// Gets a field from a given table row. The generic argument
    /// specifies what type the field should be interpreted as.
    /// Returns `Ok(F)` if the field was successfully found, `Err`
    /// if not.
    ///
    /// * `table` - The table name to get a field from.
    /// * `row_id` - The ID of the row to get a field from.
    /// * `field_name` - The field name to get.
    pub fn get_field_in_table_row<F>(
        &self,
        table: impl Into<String>,
        row_id: RowId,
        field_name: impl Into<String>,
    ) -> Result<F>
    where
        F: FromSql,
    {
        let connection = self.get_connection_if_exists()?;

        let mut select = connection.prepare(
            format!(
                "SELECT {} FROM {} WHERE id = ?1",
                field_name.into(), table.into()
            )
            .as_str(),
        )?;

        select
            .query_one([row_id.0], |t| t.get(0))
            .map_err(|e| {
                Error::DatabaseError(e.to_string())
            })
    }

    /// Removes a row from a table. Returns `Ok` if the row was
    /// successfully removed, `Err` otherwise.
    ///
    /// * `table` - The name of the table to remove a row from.
    /// * `row_id` - The row ID to remove.
    pub fn remove_row_in_table(
        &self,
        table: impl Into<String>,
        row_id: RowId,
    ) -> Result<()> {
        let connection = self.get_connection_if_exists()?;

        connection.execute(
            format!(
                "DELETE FROM {} WHERE id = ?1",
                table.into()
            )
            .as_str(),
            [row_id.0],
        )?;

        Ok(())
    }
}

/// Used to identify a unique row in a table.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RowId(pub i64);

impl Display for RowId {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{}", self.0)?;
        Ok(())
    }
}

/// A trait implemented by a struct to define a table's columns.
/// Use the `framework_derive_macros::TableRow` derive macro to
/// implement this trait automatically.
pub trait TableRow: Sized + std::fmt::Debug {
    /// Called when a connection is opened. Executes the appropriate
    /// SQL query to create the table if it does not exist.
    fn setup(
        connection: &mut Connection,
        table_name: String,
    ) -> Result<()>;

    /// Creates an instance of this struct based on a row from the
    /// given table.
    ///
    /// * `db_connection` - A connection to the SQL database.
    /// * `table_name` - The name of the table to get data from.
    /// * `row_id` - The row ID to get data from.
    fn from_table_row(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
    ) -> Result<Self>;
    
    /// Pushes a record into a `tabled::TableBuilder`
    /// containing the names of each field.
    /// Called once at the beginning, then
    /// `push_tabled_record` is called for each
    /// subsequent row.
    fn push_tabled_header(builder: &mut TabledBuilder);

    /// Pushes a record into a `tabled::TableBuilder`
    /// containing the values of each field.
    /// Called for each row in a table, after
    /// `push_tabled_header`.
    fn push_tabled_record(builder: &mut TabledBuilder, db_connection: &DbConnection, table_name: String, row_id: RowId);
}

/// Contains data about a single training client.
#[derive(TableRow, Debug)]
pub struct Client {
    // The client's name.
    name: String,
}

impl Client {
    /// Gets a client's name.
    pub fn name(&self) -> &String {
        &self.name
    }
}

/// A trait for types stored in a SQL database. Useful
/// for translating data from SQL to Rust.
pub trait TableField {
    /// Gets the SQL data type (`INTEGER`, `TEXT`, etc)
    fn sql_type() -> &'static str;

    /// Creates an instance of the data type from a SQL table.
    ///
    /// * `db_connection` - A connection to the SQL database.
    /// * `table_name` - The name of the table to get data from.
    /// * `row_id` - The row ID to get data from.
    /// * `field_name` - The name of the field to get data from.
    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self>
    where
        Self: Sized;
}


/// Stores information about a trainer. Useful for holding company details.
#[derive(TableRow, Debug)]
pub struct Trainer {
    name: String,
    company_name: String,
    address: String,
    email: String,
    phone: String,
}

impl Trainer {
    /// Gets the trainer's name.
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Gets the trainer's company name.
    pub fn company_name(&self) -> &String {
        &self.company_name
    }

    /// Gets the trainer's address.
    pub fn address(&self) -> &String {
        &self.address
    }

    /// Gets the trainer's email address.
    pub fn email(&self) -> &String {
        &self.email
    }

    /// Gets the trainer's phone number.
    pub fn phone(&self) -> &String {
        &self.phone
    }
}

/// A configuration for a SQL table. Used when opening a
/// database connection to ensure all needed tables exist.
#[derive(Clone)]
pub struct TableConfig {
    /// The name of the table.
    pub table_name: String,

    /// The setup function for the table. Generally should
    /// be the `TableRow::setup` implementation for the
    /// row type.
    pub setup_fn: TableSetupFn,

    /// See `PushTabledHeaderFn`.
    pub push_tabled_header_fn: PushTabledHeaderFn,

    /// See `PushTabledRecordFn`.
    pub push_tabled_record_fn: PushTabledRecordFn,
}

/// A pointer to a function used to set up a table. Generally
/// points to the `TableRow::setup` implementation for a
/// given row type.
pub type TableSetupFn =
    fn(&mut Connection, String) -> Result<()>;

/// A pointer to a function used to push a header
/// into a `tabled` builder. Generally points to
/// the `TableRow::push_tabled_header` implementation
/// for the row type.
pub type PushTabledHeaderFn = fn (&mut TabledBuilder);

/// A pointer to a function used to push a record
/// for a table row into a `tabled` builder. Generally
/// points to the `TableRow::push_tabled_record`
/// implementation for the row type.
pub type PushTabledRecordFn = fn (&mut TabledBuilder, &DbConnection, String, RowId);

//////////////////////////////////////////////////////
// PRIVATE IMPLEMENTATION
//////////////////////////////////////////////////////

impl Plugin for DbPlugin {
    fn build(self, context: &mut Context) {
        add_db_commands(context);
        context.add_resource(EditCommandTuiState::default());
    }
}

fn add_db_commands(context: &mut Context) {
    context.add_table::<Trainer>("trainer")
        .add_table::<Client>("client");
    context
        .add_command(Command::new("db")
            .about("View and update database configuration")
            .subcommand(Command::new("info")
                .about("Prints information about the database")
            )
            .subcommand(Command::new("erase")
                .about("Erases the database")
            )
            .subcommand(
                Command::new("backup")
                    .about("Copies the database to a new file")
                    .arg(
                        Arg::new("out-file")
                            .long("out-file")
                            .required(true)
                            .help("File path to copy the database to (will be overwritten)")
                    )
            )
            .subcommand(
                Command::new("restore")
                    .about("Restores the database from a given file")
                    .arg(
                        Arg::new("file")
                            .long("file")
                            .required(true)
                            .help("File path to restore the database from")
                    )
            )
            .subcommand_required(true),
            process_db_command)
        .add_command(Command::new("new")
            .about("Add a new row to a table")
            .arg(
                Arg::new("table")
                    .long("table")
                    .required(true)
                    .help("Name of the table to add a row in")
            ),
            process_new_command
        )
        .add_command(
            Command::new("remove").alias("rm")
                .about("Removes a row from a table")
                .arg(
                    Arg::new("table")
                        .long("table")
                        .required(true)
                        .help("Name of the table to remove a row from")
                )
                .arg(
                    Arg::new("row-id")
                        .long("row-id")
                        .value_parser(clap::value_parser!(i64))
                        .required(true)
                        .help("Row ID to remove")
                ),
            process_remove_command
        )
        .add_command(
            Command::new("list").alias("ls")
                .about("Lists the rows of a table")
                .arg(
                    Arg::new("table")
                        .long("table")
                        .required(true)
                        .help("Name of the table to list rows from")
                ),
            process_list_command
        )
        .add_command(
            Command::new("set")
               .about("Sets a field in the given table and row.")
                .arg(
                    Arg::new("table")
                        .long("table")
                        .required(true)
                        .help("Name of the table to to modify")
                )
                .arg(
                    Arg::new("row-id")
                        .long("row-id")
                        .value_parser(clap::value_parser!(i64))
                        .required(true)
                        .help("Row ID to modify")
                )
                .arg(
                    Arg::new("field")
                        .long("field")
                        .required(true)
                        .help("Name of the field to modify")
                )
                .arg(
                    Arg::new("value")
                        .long("value")
                        .required(true)
                        .help("Value to set the field to")
                ),
            process_set_command
        )
        .add_command(
            Command::new("edit")
                .about("Edits a table row in TUI mode.")
                .arg(
                    Arg::new("table")
                        .long("table")
                        .required(true)
                        .help("Name of the table to edit")
                ),
            process_edit_command
        );
}

fn erase_db(
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
    db_connection.delete_db()?;
    Ok(CommandResponse::default())
}

fn process_db_command(
    context: &mut Context,
    matches: &ArgMatches,
) -> Result<CommandResponse> {
    match matches.subcommand() {
        Some(("info", _)) => {
            let db_connection = context.db_connection().unwrap();
            process_db_info_command(db_connection)
        }
        Some(("erase", _)) => {
            let db_connection = context.db_connection().unwrap();
            erase_db(db_connection)
        }
        Some(("backup", _)) => {
            Ok(CommandResponse::default())
        }
        Some(("restore", _)) => {
            Ok(CommandResponse::default())
        }
        _ => Ok(CommandResponse::default()),
    }
}

fn process_new_command(
    context: &mut Context,
    arg_matches: &ArgMatches, 
) -> Result<CommandResponse> {
    let db_connection = context.db_connection().unwrap();
    let table: &String = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");
    let new_row_id = db_connection
        .new_row_in_table(table.clone())
        .expect("couldn't insert new row!");

    Ok(CommandResponse::new(
        format!("Inserted new row (id: {}) in table {}.", new_row_id, table)
    ))
}

fn process_set_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> Result<CommandResponse> {
    let db_connection = context.db_connection().unwrap();
    let table = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");
    let row_id = RowId(
        *arg_matches
            .get_one::<i64>("row-id")
            .expect("Missing required argument"),
    );
    let field = arg_matches
        .get_one::<String>("field")
        .expect("Missing required argument");
    let value = arg_matches
        .get_one::<String>("value")
        .expect("Missing required argument");

    db_connection
        .set_field_in_table(
            table.clone(),
            row_id,
            field.clone(),
            value.clone(),
        )
        .expect("couldn't set field!");
    Ok(CommandResponse::default())
}

fn process_list_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> Result<CommandResponse> {
    let db_connection = context.db_connection().unwrap();
    let table = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");

    let ids = db_connection
        .get_table_row_ids(table.clone())
        .expect("couldn't get table row ids");

    let response_text = if ids.is_empty() {
        format!("No entries in table {}.", table)
    } else {
        let Some(table_config) = db_connection.tables.iter().find(|t| t.table_name == *table) else {
            return Err(Error::UnknownError);
        };

        let mut tabled_builder = TabledBuilder::default();
        (table_config.push_tabled_header_fn)(&mut tabled_builder);
        for id in ids {
            (table_config.push_tabled_record_fn)(&mut tabled_builder, db_connection, table.to_string(), RowId(id))
        }
        tabled_builder.build().to_string()
    };
    Ok(CommandResponse::new(response_text))
}

fn process_remove_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> Result<CommandResponse> {
    let db_connection = context.db_connection().unwrap();
    let table = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");
    let row_id = arg_matches
        .get_one::<i64>("row-id")
        .expect("Missing required argument");

    db_connection
        .remove_row_in_table(
            table.clone(),
            RowId(*row_id),
        )
        .expect("Couldn't remove row from table");

    Ok(CommandResponse::default())
}

fn process_db_info_command(db_connection: &mut DbConnection) -> Result<CommandResponse> {
    let mut response_text = String::default();
    if db_connection.is_open() {
        response_text += "Database connection open.\n";
        if let Some(db_path) = db_connection.db_path() {
            response_text += format!("Database path: {:?}", db_path).as_str();
        } else {
            response_text += "No database path (in-memory connection)";
        }
    } else {
        response_text += "No database connection open.";
    }

    Ok(CommandResponse::new(response_text))
}

#[derive(Default)]
struct EditCommandTuiState {
    table: String
}

impl Resource for EditCommandTuiState {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

fn process_edit_command(
    context: &mut Context,
    arg_matches: &ArgMatches, 
) -> Result<CommandResponse> {
    if let Some(state) = context.get_resource_mut::<EditCommandTuiState>() {
        state.table = arg_matches.get_one::<String>("table").unwrap().into();
    }
    Ok(CommandResponse::new("Starting TUI session...").request_tui(render_edit_tui, update_edit_tui))
}

fn render_edit_tui(context: &mut Context, frame: &mut ratatui::Frame) {
    let table = if let Some(state) = context.get_resource_mut::<EditCommandTuiState>() {
        Some(state.table.clone())
    } else { None };

    let title = format!("Editing table: {}", table.clone().unwrap_or("<err>".into()));

    let db_connection = context.db_connection().unwrap();

    let row_ids = db_connection.get_table_row_ids(table.unwrap_or_default()).unwrap();

    let body = format!("{} rows", row_ids.len());

    let paragraph = Paragraph::new(body)
        .centered()
        .block(
            Block::bordered()
                .title(Line::from(title.bold()).centered())
                .title_bottom(Line::from("Exit <Q> Up <A> Down <Z>".bold()).centered())
        );
        
    frame.render_widget(paragraph, frame.area());
}

fn update_edit_tui(context: &mut Context, tui_state: &mut TuiState, ev: &crossterm::event::Event) {
    match ev {
        Event::Key(key_event) => {
            if key_event.kind == KeyEventKind::Press {
                if key_event.code == KeyCode::Char('q') {
                    tui_state.request_quit();
                }
                if let Some(state) = context.get_resource_mut::<EditCommandTuiState>() {
                
                }
            }
        }
        _ => { }
    }
}

impl DbConnection {
    // opens a db connection at the default db path
    pub(crate) fn open_default(
        table_configs: Vec<TableConfig>,
    ) -> Result<Self> {
        assert!(!cfg!(test));
        let db_path = Self::get_default_db_path()?;
        Self::open_from_path(&db_path, table_configs)
    }

    // opens a db connection at a test db path
    pub(crate) fn open_test(
        table_configs: Vec<TableConfig>,
    ) -> Result<Self> {
        Self::open_in_memory(table_configs)
    }

    fn get_default_db_path() -> Result<PathBuf> {
        // get the cache directory for this
        // application
        let dirs = directories::ProjectDirs::from(
            "",
            "",
            "training_assistant",
        )
        // translate to an error if it failed
        .ok_or(Error::FileError(
            "Failed to get data directory".into(),
        ))?;

        Ok(dirs.data_dir().join("data/data.db"))
    }

    fn open_in_memory(
        table_configs: Vec<TableConfig>,
    ) -> Result<Self> {
        // create the database in memory
        let mut connection =
            Connection::open_in_memory()?;

        // run the connection setup to ensure
        // tables exist
        Self::setup_connection(
            &mut connection,
            &table_configs,
        )?;

        Ok(Self {
            connection: Some(connection),
            db_path: None,
            tables: table_configs
        })
    }

    // opens a db connection from the specified path
    fn open_from_path(
        path: &PathBuf,
        table_configs: Vec<TableConfig>,
    ) -> Result<Self> {
        // create the directories leading to 
        // the db path
        fs::create_dir_all(path.parent().unwrap())?;
        
        // open the database connection
        let mut connection =
            Connection::open(path.clone())?;

        // run the connection setup to ensure
        // tables exist
        Self::setup_connection(
            &mut connection,
            &table_configs,
        )?;

        Ok(DbConnection {
            connection: Some(connection),
            db_path: Some(path.clone()),
            tables: table_configs
        })
    }

    fn setup_connection(
        connection: &mut Connection,
        table_configs: &Vec<TableConfig>,
    ) -> Result<()> {
        for table_config in table_configs {
            (table_config.setup_fn)(
                connection,
                table_config.table_name.clone(),
            )?;
        }
        Ok(())
    }

    fn get_connection_if_exists(&self) -> Result<&Connection> {
        if let Some(connection) = &self.connection {
            Ok(connection)
        } else {
            Err(Error::NoConnectionError)
        }
    }
}

impl TableField for String {
    fn sql_type() -> &'static str {
        "TEXT"
    }
    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        db_connection.get_field_in_table_row::<String>(
            table_name, row_id, field_name,
        )
    }
}

impl TableField for i32 {
    fn sql_type() -> &'static str {
        "INTEGER"
    }
    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        db_connection.get_field_in_table_row::<i32>(
            table_name, row_id, field_name,
        )
    }
}

impl TableField for i64 {
    fn sql_type() -> &'static str {
        "INTEGER"
    }
    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        db_connection.get_field_in_table_row::<i64>(
            table_name, row_id, field_name,
        )
    }
}

impl TableField for RowId {
    fn sql_type() -> &'static str {
        "INTEGER"
    }
    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        Ok(Self(
            db_connection
                .get_field_in_table_row::<i64>(
                    table_name, row_id, field_name,
                )?,
        ))
    }
}

impl TableField for Vec<RowId> {
    fn sql_type() -> &'static str {
        "TEXT"
    }
    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        let s = db_connection
            .get_field_in_table_row::<String>(
                table_name, row_id, field_name,
            )?;
        let mut vec = Vec::new();
        for v in s.split(',') {
            let i: i64 = v.parse().unwrap();
            vec.push(RowId(i));
        }
        Ok(vec)
    }
}

#[cfg(test)]
mod test {
    use crate::prelude::*;
    use framework_derive_macros::TableRow;

    #[derive(Clone)]
    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn build(self, context: &mut Context) {
            context.add_table::<TestTableRow>("foo");
        }
    }

    #[derive(TableRow, Debug)]
    struct TestTableRow {
        bar: String,
    }

    #[test]
    fn db_table_ops_test() -> Result<()> {
        // create a context and add our test plugin
        let mut context = Context::new();
        context.add_plugin(TestPlugin);
        context.in_memory_db(true);

        context.startup()?;

        // open the db connection
        let mut db_connection =
            context.db_connection()?;

        // check the db connection is open
        assert!(db_connection.is_open());

        // test db connection shouldn't have a file path
        assert!(db_connection.db_path().is_none());
        
        // insert a row and check the inserted row is 
        // row 1
        // (the table was empty)
        let inserted_row =
            db_connection.new_row_in_table("foo")?;
        assert_eq!(inserted_row.0, 1);
       
        // check the table row IDs returned are just
        // our newly created row
        let table_row_ids = db_connection.get_table_row_ids("foo")?;
        assert_eq!(table_row_ids, vec![1]);
       
        // set a field in the created row
        db_connection.set_field_in_table(
            "foo",
            inserted_row,
            "bar",
            "foobar",
        )?;

        // ensure the field matches
        let field = db_connection.get_field_in_table_row::<String>(
            "foo", 
            inserted_row, 
            "bar"
        )?;
        assert_eq!(field, "foobar");

        // get the table row and ensure the field matches
        let table_row = TestTableRow::from_table_row(
            &mut db_connection,
            "foo".into(),
            inserted_row,
        )?;
        assert_eq!(table_row.bar, "foobar");

        // delete the row
        db_connection.remove_row_in_table("foo", inserted_row)?;

        // ensure the table row IDs are empty
        let table_row_ids_2 = db_connection.get_table_row_ids("foo")?;
        assert_eq!(table_row_ids_2.len(), 0);

        // delete the db. this one is in memory, so it
        // should just close the connection
        db_connection.delete_db()?;

        Ok(())
    }

    #[test]
    fn db_commands_test() -> Result<()> {
        let mut context = Context::new();
        context.add_plugin(DbPlugin);
        context.in_memory_db(true);

        context.startup()?;

        let info_response = context.execute("db info")?;
        assert!(info_response.text().is_some());
        assert!(info_response.text().unwrap() == "Database connection open.\nNo database path (in-memory connection)");

        assert_eq!(context.db_connection()?.get_table_row_ids("trainer")?, vec![]);
        let new_response = context.execute("new --table=trainer")?;
        assert!(new_response.text().is_some());
        assert_eq!(new_response.text().unwrap(), "Inserted new row (id: 1) in table trainer.");
        assert_eq!(context.db_connection()?.get_table_row_ids("trainer")?, vec![1]); 

        let list_response = context.execute("list --table=trainer")?;
        println!("{}", list_response.text().unwrap());
        assert_eq!(list_response.text().unwrap(), 
            "+----+------+--------------+---------+-------+-------+\n\
            | ID | name | company_name | address | email | phone |\n\
            +----+------+--------------+---------+-------+-------+\n\
            | 1  | Err  |              |         |       |       |\n\
            +----+------+--------------+---------+-------+-------+"
        );
        context.execute("db erase")?;
        assert!(!context.db_connection()?.is_open());

        Ok(())
    }
}
