use crate::{
    Error, Result,
    context::{Context, Plugin, Resource},
};
use rusqlite::{
    Connection, ToSql, params, types::FromSql,
};
use std::fs;
use std::path::PathBuf;
use std::fmt::{Display, Formatter};
use tabled::{builder::Builder as TabledBuilder};
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

impl Resource for DbConnection {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
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
            return Err(Error::new("no active db connection"));
        };

        // close the connection
        connection.close().map_err(|e| e.1)?;

        // erase the database file (if it exists)
        if let Some(db_path) = &self.db_path {
            std::fs::remove_file(db_path.clone())
                .map_err(|e| {
                    Error::new(format!("couldn't erase db file: {}", e.to_string()))
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
                Error::new(e.to_string())
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

    /// Gets the `TableConfigs` this `Context` was created
    /// with.
    pub fn tables(&self) -> &Vec<TableConfig> {
        &self.tables
    }
}

/// Used to identify a unique row in a table.
// TODO: don't love that this implements Default, was necessary to implement FieldType
#[derive(Copy, Clone, Debug, Eq, Default, PartialEq)]
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

    /// Gets all the field names of this row type.
    // TODO: remove this in favor of field_types
    fn field_names() -> Vec<String>;

    /// Gets all the fields of a row from a table, as strings.
    fn get_fields_as_strings(db_connection: &DbConnection, table_name: String, row_id: RowId) -> Vec<String>;

    fn field_types() -> Vec<FieldTypeInfo>;
}

#[derive(Debug)]
pub struct FieldTypeInfo {
    pub name: String,
    pub type_id: std::any::TypeId
}

impl FieldTypeInfo {
    pub fn name(&self) -> &String {
       &self.name 
    }

    pub fn type_id(&self) -> std::any::TypeId {
        self.type_id
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

    /// Formats a display string (for use in table output) from this field.
    ///
    /// * `args` - A struct of arguments used to format the display string.
    fn to_display_string(&self, args: TableFieldDisplayStringArgs) -> String;
}

/// Arguments for `TableField::to_display_string`.
pub struct TableFieldDisplayStringArgs<'a> {
    /// A connection to the database.
    pub db_connection: &'a DbConnection,
    
    /// If applicable, a table and column to show for a RowId, instead of its numeric id.
    pub display_table: Option<(String, String)>
}

/// A configuration for a SQL table. Used when opening a
/// database connection to ensure all needed tables exist.
// TODO: this should have a new<T> automatically setting the function pointers
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

    /// See `FieldNamesFn`.
    pub field_names_fn: FieldNamesFn,

    pub field_types_fn: FieldTypesFn,

    /// See `GetFieldsAsStringsFn`.
    pub get_fields_as_strings_fn: GetFieldsAsStringsFn
}

impl TableConfig {
    /// Creates a new TableConfig with the given row type (`T`) and the given table name.
    ///
    /// * `table_name` - The name to give the table.
    pub fn new<T>(table_name: impl Into<String>) -> Self
        where T: TableRow 
    {
        Self {
            table_name: table_name.into(),
            setup_fn: T::setup,
            push_tabled_header_fn: T::push_tabled_header,
            push_tabled_record_fn: T::push_tabled_record,
            field_names_fn: T::field_names,
            field_types_fn: T::field_types,
            get_fields_as_strings_fn: T::get_fields_as_strings,
        }
    }
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

/// A pointer to a function used to get the field names
/// for a table's row type. Generally points to the
/// `TableRow::field_names` implementation for the
/// row type.
pub type FieldNamesFn =
    fn() -> Vec<String>;

pub type FieldTypesFn =
    fn() -> Vec<FieldTypeInfo>;

/// A pointer to a function used to get the fields from a table row as strings.
/// Generally points to the `TableRow::get_fields_as_strings` implementation for
/// the row type.
pub type GetFieldsAsStringsFn = fn(&DbConnection, String, RowId) -> Vec<String>;

/// A resource to hold the tables that should be requested on startup.
#[derive(Default)]
pub struct DbTableConfigs {
    configs: Vec<TableConfig>
}

// TODO: autogenerate this
impl Resource for DbTableConfigs {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

impl DbTableConfigs {
    fn add_table(&mut self, table: TableConfig) {
        self.configs.push(table);
    }
}

/// An extension to `Context` to add database functionality acessible directly from the context.
pub trait DbContextExt {
    /// Gets the database connection from a `Context`, if it is active. Returns `Err` if not.
    fn db_connection(&mut self) -> Result<&mut DbConnection>;

    /// Adds a new table configuration. Must be called before `Context::startup`.
    ///
    /// * `table` - The table configuration to add.
    fn add_table(&mut self, table: TableConfig) -> &mut Context;
}

impl DbContextExt for Context {
    fn db_connection(&mut self) -> Result<&mut DbConnection> {
        self.get_resource_mut::<DbConnection>().ok_or(Error::new("no active db connection"))
    }

    fn add_table(&mut self, table: TableConfig) -> &mut Context {
        if !self.has_resource::<DbTableConfigs>() {
            let mut configs = DbTableConfigs::default();
            configs.add_table(table);
            self.add_resource(configs);
        } else {
            // this unwrap is safe, as we know this resource exists
            self.get_resource_mut::<DbTableConfigs>().unwrap().add_table(table);
        }

        self
    }
}

//////////////////////////////////////////////////////
// PRIVATE IMPLEMENTATION
//////////////////////////////////////////////////////

// TODO: is this plugin needed?
impl Plugin for DbPlugin {
    fn build(self, _: &mut Context) -> Result<()> {
        Ok(())
    }
}

impl DbConnection {
    // opens a db connection at the default db path
    pub(crate) fn open_default(context: &Context) -> Result<Self> {
        assert!(!cfg!(test));
        let db_path = Self::get_default_db_path()?;
        let table_configs = match context.get_resource::<DbTableConfigs>() {
            Some(t) => t.configs.clone(),
            None => Vec::new()
        };
        Self::open_from_path(&db_path, table_configs)
    }

    // opens a db connection at a test db path
    pub(crate) fn open_test(context: &Context) -> Result<Self> {
        let table_configs = match context.get_resource::<DbTableConfigs>() {
            Some(t) => t.configs.clone(),
            None => Vec::new()
        };
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
        .ok_or(Error::new(
            "Failed to get data directory",
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
            Err(Error::new("no active db connection"))
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

    fn to_display_string(&self, _: TableFieldDisplayStringArgs) -> String {
        format!("{:?}", self)
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
    fn to_display_string(&self, _: TableFieldDisplayStringArgs) -> String {
        format!("{:?}", self)
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
    fn to_display_string(&self, _: TableFieldDisplayStringArgs) -> String {
        format!("{:?}", self)
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
    fn to_display_string(&self, args: TableFieldDisplayStringArgs) -> String {
        if let Some(display_table) = args.display_table {
            let display_data = args.db_connection.get_field_in_table_row::<String>(display_table.0.clone(), *self, display_table.1.clone());
            if let Ok(data) = display_data {
                data
            } else { 
                format!("invalid row {}, column {:?} in table {:?}", self.0, display_table.1, display_table.0)
            }
        } else {
            format!("{:?}", self)
        }
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
    fn to_display_string(&self, _: TableFieldDisplayStringArgs) -> String {
        format!("{:?}", self)
    }
}

impl TableField for chrono::NaiveDate {
    fn sql_type() -> &'static str {
        "TEXT"
    }
    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        let s = db_connection.get_field_in_table_row::<String>(
            table_name, row_id, field_name,
        )?;
        if let Ok(date) = s.parse::<chrono::NaiveDate>() {
            Ok(date)
        } else {
            Err(Error::new("failed to parse date"))
        }
    }
    fn to_display_string(&self, _: TableFieldDisplayStringArgs) -> String {
        format!("{:?}", self)
    }
}

impl<T> TableField for Option<T>
    where T: FromSql + std::fmt::Debug
{
    fn sql_type() -> &'static str {
        "TEXT"
    }

    fn from_table_field(
        db_connection: &DbConnection,
        table_name: String,
        row_id: RowId,
        field_name: String,
    ) -> Result<Self> {
        let s = db_connection.get_field_in_table_row::<T>(table_name, row_id, field_name);
        if let Ok(v) = s {
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }
    fn to_display_string(&self, _: TableFieldDisplayStringArgs) -> String {
        format!("{:?}", self)
    }
}

impl FromSql for RowId {
    fn column_result(value: rusqlite::types::ValueRef) -> rusqlite::types::FromSqlResult<Self> {
        if value.data_type() == rusqlite::types::Type::Integer {
            Ok(RowId(value.as_i64().unwrap()))
        } else {
            Err(rusqlite::types::FromSqlError::InvalidType)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::prelude::*;
    use framework_derive_macros::TableRow;
    extern crate self as framework;

    #[derive(Clone)]
    struct TestPlugin;

    impl Plugin for TestPlugin {
        fn build(self, context: &mut Context) -> Result<()> {
            context.add_table(TableConfig::new::<TestTableRow>("foo"));
            Ok(())
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
        context.add_plugin(TestPlugin)?;
        context.in_memory_db(true);

        context.startup()?;

        // open the db connection
        let mut db_connection =
            context.db_connection()?;

        // check the db connection is open
        assert!(db_connection.is_open());

        // test db connection shouldn't have a file path
        assert!(db_connection.db_path().is_none());
        
        // test field type info
        let field_types = (db_connection.tables.iter()
            .find(|t| t.table_name == "foo").ok_or(Error::default())?
            .field_types_fn)();
        assert_eq!(field_types.len(), 1);
        assert_eq!(field_types[0].name(), "bar");
        assert_eq!(field_types[0].type_id(), std::any::TypeId::of::<String>());

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
}
