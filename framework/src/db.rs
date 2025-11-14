use crate::{
    Error, Result,
    context::{CommandResponse, Context, Plugin},
};
use clap::{Arg, ArgMatches, Command};
use framework_derive_macros::TableRow;
use rusqlite::{
    Connection, ToSql, params, types::FromSql,
};
use std::fs;
use std::path::PathBuf;

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
    ///     implement `ToSql`)
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
        table: String,
    ) -> Result<Vec<i64>> {
        let connection = self.get_connection_if_exists()?;

        let mut select = connection.prepare(
            format!("SELECT id FROM {}", table)
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
        table: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<F>
    where
        F: FromSql,
    {
        let connection = self.get_connection_if_exists()?;

        let mut select = connection.prepare(
            format!(
                "SELECT {} FROM {} WHERE id = ?1",
                field_name, table
            )
            .as_str(),
        )?; //, params![row_id.0]);

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
        table: String,
        row_id: RowId,
    ) -> Result<()> {
        let connection = self.get_connection_if_exists()?;

        connection.execute(
            format!(
                "DELETE FROM {} WHERE id = ?1",
                table
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

/// A trait implemented by a struct to define a table's columns.
/// Use the `framework_derive_macros::TableRow` derive macro to
/// implement this trait automatically.
pub trait TableRow: Sized {
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
        db_connection: &mut DbConnection,
        table_name: String,
        row_id: RowId,
    ) -> Result<Self>;
}

/// Contains data about a single training client.
#[derive(TableRow)]
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
        db_connection: &mut DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self>
    where
        Self: Sized;
}


/// Stores information about a trainer. Useful for holding company details.
#[derive(TableRow)]
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
pub struct TableConfig {
    /// The name of the table.
    pub table_name: String,

    /// The setup function for the table. Generally should
    /// be the `TableRow::setup` implementation for the
    /// row type.
    pub setup_fn: TableSetupFn,
}

/// A pointer to a function used to set up a table. Generally
/// points to the `TableRow::setup` implementation for a
/// given row type.
pub type TableSetupFn =
    fn(&mut Connection, String) -> Result<()>;

//////////////////////////////////////////////////////
// PRIVATE IMPLEMENTATION
//////////////////////////////////////////////////////

impl Plugin for DbPlugin {
    fn build(self, context: &mut Context) {
        add_db_commands(context);
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
        );
}

fn erase_db(
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
    db_connection.delete_db()?;
    Ok(CommandResponse::default())
}

fn process_db_command(
    matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
    let response = match matches.subcommand() {
        Some(("info", _)) => {
            CommandResponse::default()
        }
        Some(("erase", _)) => erase_db(db_connection)?,
        Some(("backup", _)) => {
            CommandResponse::default()
        }
        Some(("restore", _)) => {
            CommandResponse::default()
        }
        _ => CommandResponse::default(),
    };

    Ok(response)
}

fn process_new_command(
    arg_matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
    let table: &String = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");
    db_connection
        .new_row_in_table(table.clone())
        .expect("couldn't insert new row!");

    Ok(CommandResponse::default())
}

fn process_set_command(
    arg_matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
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
    arg_matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
    let table = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");

    let ids = db_connection
        .get_table_row_ids(table.clone())
        .expect("couldn't get table row ids");

    if ids.is_empty() {
        println!("No entries in table {}", table);
    } else {
        for id in ids {
            println!("{}", id);
        }
    }
    Ok(CommandResponse::default())
}

fn process_remove_command(
    arg_matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
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

impl DbConnection {
    // opens a db connection at the default db path
    pub(crate) fn open_default(
        table_configs: &Vec<TableConfig>,
    ) -> Result<Self> {
        let db_path = Self::get_default_db_path()?;
        Self::open_from_path(&db_path, table_configs)
    }

    // opens a db connection at a test db path
    pub(crate) fn open_test(
        table_configs: &Vec<TableConfig>,
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
        table_configs: &Vec<TableConfig>,
    ) -> Result<Self> {
        // create the database in memory
        let mut connection =
            Connection::open_in_memory()?;

        // run the connection setup to ensure
        // tables exist
        Self::setup_connection(
            &mut connection,
            table_configs,
        )?;

        Ok(Self {
            connection: Some(connection),
            db_path: None,
        })
    }

    // opens a db connection from the specified path
    fn open_from_path(
        path: &PathBuf,
        table_configs: &Vec<TableConfig>,
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
            table_configs,
        )?;

        Ok(DbConnection {
            connection: Some(connection),
            db_path: Some(path.clone()),
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
        db_connection: &mut DbConnection,
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
        db_connection: &mut DbConnection,
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
        db_connection: &mut DbConnection,
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
        db_connection: &mut DbConnection,
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
        db_connection: &mut DbConnection,
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

    #[derive(TableRow)]
    struct TestTableRow {
        bar: String,
    }

    #[test]
    fn db_test_1() -> Result<()> {
        let mut context = Context::new();
        context.add_plugin(TestPlugin);
        let mut db_connection =
            context.open_db_connection()?;
        // test db connection shouldn't have a file path
        assert!(db_connection.db_path().is_none());
        let inserted_row =
            db_connection.new_row_in_table("foo")?;
        assert_eq!(inserted_row.0, 1);
        db_connection.set_field_in_table(
            "foo",
            inserted_row,
            "bar",
            "foobar",
        )?;
        let table_row = TestTableRow::from_table_row(
            &mut db_connection,
            "foo".into(),
            inserted_row,
        )?;
        assert_eq!(table_row.bar, "foobar");
        Ok(())
    }
}
