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

/// A connection to the underlying SQLite database.
pub struct DatabaseConnection {
    connection: Option<Connection>,
    db_path: PathBuf,
}

impl DatabaseConnection {
    /// Returns true if the connection is open.
    pub fn is_open(&self) -> bool {
        self.connection.is_some()
    }

    /// Gets the path of the database file opened by this connection.
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Deletes the database file. Requires a currently open connection, and will close it on successful
    /// deletion.
    pub fn delete_db(&mut self) -> Result<()> {
        let Some(connection) = self.connection.take()
        else {
            return Err(Error::NoConnectionError);
        };
        connection.close().map_err(|e| {
            Error::DatabaseError(e.1.to_string())
        })?;
        std::fs::remove_file(self.db_path.clone())
            .map_err(|e| {
                Error::FileError(e.to_string())
            })?;
        self.db_path = PathBuf::default();
        Ok(())
    }

    /// Inserts a new empty row into a table, setting
    /// fields to default values.
    ///
    /// * `table` - The table name to insert a row into.
    pub fn new_row_in_table(
        &mut self,
        table: impl Into<String>,
    ) -> Result<()> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(
            format!(
                "INSERT INTO {} DEFAULT VALUES;",
                table.into()
            )
            .as_str(),
            [],
        )?;

        Ok(())
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
        table: String,
        row_id: RowId,
        field: String,
        value: V,
    ) -> Result<()>
    where
        V: ToSql,
    {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(
            format!(
                "UPDATE {} SET {} = ?1 WHERE id = ?2",
                table, field
            )
            .as_str(),
            params![value, row_id.0],
        )?;

        Ok(())
    }

    pub fn get_table_row_ids(
        &self,
        table: String,
    ) -> Result<Vec<i64>> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        let mut select = connection.prepare(
            format!("SELECT id FROM {}", table)
                .as_str(),
        )?;

        Ok(select
            .query_map([], |row| row.get(0))?
            .filter_map(|c| c.ok())
            .collect())
    }

    pub fn get_field_in_table_row<F>(
        &self,
        table: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<F>
    where
        F: FromSql,
    {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

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

    pub fn remove_row_in_table(
        &self,
        table: String,
        row_id: RowId,
    ) -> Result<()> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

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
        let db_path = Self::get_test_db_path()?;
        Self::open_from_path(&db_path, table_configs)
    }

    fn get_default_db_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from(
            "",
            "",
            "training_assistant",
        )
        .ok_or(Error::FileError(
            "Failed to get data directory".into(),
        ))?;
        Ok(dirs.data_dir().join("data/data.db"))
    }

    fn get_test_db_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from(
            "",
            "",
            "training_assistant",
        )
        .ok_or(Error::FileError(
            "Failed to get data directory".into(),
        ))?;
        Ok(dirs.data_dir().join("data_test/data.db"))
    }

    // opens a db connection from the specified path
    fn open_from_path(
        path: &PathBuf,
        table_configs: &Vec<TableConfig>,
    ) -> Result<Self> {
        println!(
            "opening database connection at {:?}",
            path
        );

        fs::create_dir_all(path.parent().unwrap())?;
        let mut connection =
            Connection::open(path.clone())?;

        for table_config in table_configs {
            (table_config.setup_fn)(
                &mut connection,
                table_config.table_name.clone(),
            )?;
        }

        Ok(DatabaseConnection {
            connection: Some(connection),
            db_path: path.clone(),
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RowId(pub i64);

pub trait TableRow: Sized {
    fn setup(
        connection: &mut Connection,
        table_name: String,
    ) -> Result<()>;

    fn from_table_row(
        db_connection: &mut DatabaseConnection,
        table_name: String,
        row_id: RowId,
    ) -> Result<Self>;
}

#[derive(TableRow)]
pub struct Client {
    pub name: String,
}

impl Client {
    pub fn name(&self) -> &String {
        &self.name
    }
}

pub trait FieldType {
    fn sql_type() -> &'static str;
    fn from_table_field(
        db_connection: &mut DatabaseConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self>
    where
        Self: Sized;
}

impl FieldType for String {
    fn sql_type() -> &'static str {
        "TEXT"
    }
    fn from_table_field(
        db_connection: &mut DatabaseConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        db_connection.get_field_in_table_row::<String>(
            table_name, row_id, field_name,
        )
    }
}

impl FieldType for i32 {
    fn sql_type() -> &'static str {
        "INTEGER"
    }
    fn from_table_field(
        db_connection: &mut DatabaseConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        db_connection.get_field_in_table_row::<i32>(
            table_name, row_id, field_name,
        )
    }
}

impl FieldType for i64 {
    fn sql_type() -> &'static str {
        "INTEGER"
    }
    fn from_table_field(
        db_connection: &mut DatabaseConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        db_connection.get_field_in_table_row::<i64>(
            table_name, row_id, field_name,
        )
    }
}

impl FieldType for RowId {
    fn sql_type() -> &'static str {
        "INTEGER"
    }
    fn from_table_field(
        db_connection: &mut DatabaseConnection,
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

impl FieldType for Vec<RowId> {
    fn sql_type() -> &'static str {
        "TEXT"
    }
    fn from_table_field(
        db_connection: &mut DatabaseConnection,
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

#[derive(TableRow)]
pub struct Trainer {
    pub name: String,
    pub company_name: String,
    pub address: String,
    pub email: String,
    pub phone: String,
}

pub struct TableConfig {
    pub table_name: String,
    pub setup_fn: TableSetupFn,
}

pub type TableSetupFn =
    fn(&mut Connection, String) -> Result<()>;

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use std::fs;

    #[test]
    fn open_connection_test() -> Result<()> {
        let tables = Vec::new();
        let conn =
            DatabaseConnection::open_test(&tables)?;
        assert!(conn.is_open());
        assert!(fs::exists(conn.db_path()).map_err(
            |e| Error::FileError(format!(
                "failed to check if db exists: {:?}",
                e.to_string()
            ))
        )?);
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct DbPlugin;

impl Plugin for DbPlugin {
    fn build(self, context: &mut Context) {
        context
            .add_command(Command::new("db")
                .about("View and update database configuration")
                .subcommand(Command::new("info")
                    .about("Prints information about the database")
                )
                .subcommand(Command::new("erase")
                    .about("Erases the database")
                )
                .subcommand(Command::new("backup")
                    .about("Copies the database to a new file")
                    .arg(
                        Arg::new("out-file")
                        .long("out-file")
                        .required(true)
                        .help("File path to copy the database to (will be overwritten)")
                    )
                )
                .subcommand(Command::new("restore")
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
            .add_table::<Trainer>("trainer")
            .add_table::<Client>("client");
        context.add_command(Command::new("new")
            .about("Add a new row to a table")
            .arg(Arg::new("table").long("table").required(true).help("Name of the table to add a row in")),
            process_new_command
        );
        context.add_command(
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
        );
        context.add_command(
            Command::new("list").alias("ls")
                .about("Lists the rows of a table")
                .arg(
                    Arg::new("table")
                    .long("table")
                    .required(true)
                    .help("Name of the table to list rows from")
                ),
            process_list_command
        );
        context.add_command(
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
}

fn erase_db(
    db_connection: &mut DatabaseConnection,
) -> Result<CommandResponse> {
    db_connection.delete_db()?;
    Ok(CommandResponse::default())
}

fn process_db_command(
    matches: &ArgMatches,
    db_connection: &mut DatabaseConnection,
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
    db_connection: &mut DatabaseConnection,
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
    db_connection: &mut DatabaseConnection,
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
    db_connection: &mut DatabaseConnection,
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
    db_connection: &mut DatabaseConnection,
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
        db_connection.new_row_in_table("foo")?;
        Ok(())
    }
}
