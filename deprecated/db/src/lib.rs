use uuid::Uuid;
use std::fs;
use rusqlite::{Connection, params, ToSql, types::FromSql};
use std::path::{Path, PathBuf};
use framework::{App, Plugin, Result, Error};
use clap::{Arg, Command, ArgMatches};

pub struct DatabaseConnection {
    connection: Option<Connection>,
    db_path: PathBuf
}

impl DatabaseConnection {
    pub fn is_open(&self) -> bool {
        self.connection.is_some()
    }
    
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    fn get_default_db_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "training_assistant")
            .ok_or(Error::FileError("Failed to get data directory".into()))?;
        Ok(dirs.data_dir().join("data/data.db"))
    }

    fn get_test_db_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "training_assistant")
            .ok_or(Error::FileError("Failed to get data directory".into()))?;
        Ok(dirs.data_dir().join("data_test/data.db"))
    }

    pub fn open_default(row_type_setup_fns: Vec<RowTypeSetupFn>) -> Result<Self> {
        let db_path = Self::get_default_db_path()?;
        Self::open_from_path(&db_path, row_type_setup_fns)
    }

    pub fn open_test(row_type_setup_fns: Vec<RowTypeSetupFn>) -> Result<Self> {
        let db_path = Self::get_test_db_path()?;
        Self::open_from_path(&db_path, row_type_setup_fns)
    }

    fn open_from_path(path: &PathBuf, row_type_setup_fns: Vec<RowTypeSetupFn>) -> Result<Self> {
        println!("opening database connection at {:?}", path);

        fs::create_dir_all(path.parent().unwrap()).map_err(|e| 
            Error::FileError(format!("failed to create dirs for path {:?}: {:?}", path, e.to_string()))
        )?;
        let mut connection = Connection::open(path.clone())
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
       
        for row_type_setup_fn in row_type_setup_fns {
            row_type_setup_fn(&mut connection)?;
        }

        Ok(DatabaseConnection {
            connection: Some(connection),
            db_path: path.clone()
        })         
    }

    pub fn delete_db(&mut self) -> Result<()> {
        let Some(connection) = std::mem::replace(&mut self.connection, None) else {
            return Err(Error::NoConnectionError);
        };
        connection.close().map_err(|e| Error::DatabaseError(e.1.to_string()))?;
        std::fs::remove_file(self.db_path.clone()).map_err(|e| Error::FileError(e.to_string()))?;
        self.db_path = PathBuf::default(); 
        Ok(())
    }

    pub fn erase(&mut self) -> Result<()> {
        if self.connection.is_none() {
            return Err(Error::NoConnectionError);
        }

        std::fs::remove_file(self.db_path.clone())
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

        self.connection = None;

        Ok(())
    }

    pub fn add_invoice(&mut self, invoice_number: String) -> Result<()> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute_batch(format!(
            "BEGIN;
            INSERT INTO invoices (invoice_number)
            VALUES
                (\"{}\");
            COMMIT;",
            invoice_number
        ).as_str())?;

        Ok(())
    }

    pub fn insert_new_into_table(&mut self, table: String) -> Result<()> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(
            format!("INSERT INTO {} DEFAULT VALUES;", table).as_str(), [])?;

        Ok(())
    }

    pub fn set_field_in_table<V>(&mut self, table: String, id: i64, field: String, value: V) -> Result<()> 
        where V : ToSql
    {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(
            format!("UPDATE {} SET {} = ?1 WHERE id = ?2", table, field).as_str(), params![value, id])?;

        Ok(())
    }

    pub fn get_table_row_ids(&self, table: String) -> Result<Vec<i64>> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };
 
        let mut select = connection.prepare(
            format!("SELECT id FROM {}", table).as_str())?;
    
        Ok(select.query_map([], |row| {
            Ok(row.get(0)?)
        })?.filter_map(|c| {c.ok()}).collect())
    }

    pub fn get_field_in_table_row<F>(&self, table: String, row_id: RowId, field_name: String) -> Result<F>
        where F: FromSql
    {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        let mut select = connection.prepare(format!("SELECT {} FROM {} WHERE id = ?1", field_name, table).as_str())?; //, params![row_id.0]);
        
        select.query_one([row_id.0], |t| {
            Ok(t.get(0)?)
        }).map_err(|e| Error::DatabaseError(e.to_string()))
    }

    pub fn remove_row_in_table(&self, table: String, row_id: RowId) -> Result<()> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(format!("DELETE FROM {} WHERE id = ?1", table).as_str(), [row_id.0])?;

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RowId(pub i64);

pub trait RowType: Sized {
    fn setup(connection: &mut Connection) -> Result<()>;

    fn from_table_row(
        db_connection: &mut DatabaseConnection,
        row_id: RowId
    ) -> Result<Self>;
}

pub struct Client {
    pub name: String
}

impl Client {
    pub fn name(&self) -> &String {
        &self.name
    }
}

impl RowType for Client {
    fn setup(connection: &mut Connection) -> Result<()> {
        connection.execute(
            "CREATE TABLE IF NOT EXISTS client(
                id   INTEGER PRIMARY KEY,
                name TEXT);",
            []
        )?;

        Ok(())
    }

    fn from_table_row(db_connection: &mut DatabaseConnection, row_id: RowId) -> Result<Self> {
        let name = db_connection.get_field_in_table_row::<String>("client".into(), row_id, "name".into())?;
        Ok(Self {
            name
        })
    }
}

pub struct Trainer {
    pub name: String,
    pub company_name: String,
    pub address: String,
    pub email: String,
    pub phone: String
}

impl RowType for Trainer {
    fn setup(connection: &mut Connection) -> Result<()> {
        connection.execute(
            "CREATE TABLE IF NOT EXISTS trainer(
                id   INTEGER PRIMARY KEY,
                name TEXT,
                company_name TEXT,
                address TEXT,
                email TEXT,
                phone TEXT);",
            []
        )?;
        Ok(())
    }

    fn from_table_row(db_connection: &mut DatabaseConnection, row_id: RowId) -> Result<Self> {
        let name = db_connection.get_field_in_table_row::<String>("trainer".into(), row_id, "name".into())?;
        let company_name = db_connection.get_field_in_table_row::<String>("trainer".into(), row_id, "company_name".into())?;
        let address = db_connection.get_field_in_table_row::<String>("trainer".into(), row_id, "address".into())?;
        let email = db_connection.get_field_in_table_row::<String>("trainer".into(), row_id, "email".into())?;
        let phone = db_connection.get_field_in_table_row::<String>("trainer".into(), row_id, "phone".into())?;

        Ok(Self {
            name,
            company_name,
            address,
            email,
            phone
        })
    }
}


pub struct Charge {
    pub date: String,
    pub description: String,
    pub amount: i32
}

impl RowType for Charge {
    fn setup(connection: &mut Connection) -> Result<()> {
        connection.execute("
            CREATE TABLE IF NOT EXISTS charge(
                id INTEGER PRIMARY KEY,
                date TEXT,
                description TEXT,
                amount INTEGER
            );",
            []
        )?;
        Ok(())
    }

    fn from_table_row(db_connection: &mut DatabaseConnection, row_id: RowId) -> Result<Self> {
        let date = db_connection.get_field_in_table_row("charge".into(), row_id, "date".into())?;   
        let description = db_connection.get_field_in_table_row("charge".into(), row_id, "description".into())?;
        let amount = db_connection.get_field_in_table_row("charge".into(), row_id, "amount".into())?;

        Ok(Self {
            date,
            description,
            amount
        })
    }
}

pub type RowTypeSetupFn = fn (&mut Connection) -> Result<()>;

#[cfg(test)]
mod tests {
    use crate::{Result, Error, DatabaseConnection};
    use std::fs;
    
    #[test] 
    fn open_connection_test() -> Result<()> {
        let mut conn = DatabaseConnection::open_test(Vec::new())?;
        assert!(conn.is_open());
        let db_path = conn.db_path().clone();
        assert!(fs::exists(conn.db_path()).map_err(|e| Error::FileError(format!("failed to check if db exists: {:?}", e.to_string())))?);
        Ok(())
    }    
}

#[derive(Default, Clone)]
pub struct DbPlugin;

impl Plugin for DbPlugin {
    fn build(self, app: &mut App) {
        app.add_command(Command::new("db")
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
            process_db_command
        );
        app.add_command(Command::new("new")
            .about("Add a new row to a table")
            .arg(Arg::new("table").long("table").required(true).help("Name of the table to add a row in")),
            process_new_command
        );
        app.add_command(
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
        app.add_command(
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
        app.add_command(
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

fn erase_db() {
   let mut conn = DatabaseConnection::open_default().expect("Couldn't open database connection");
   conn.delete_db();
}

fn process_db_command(matches: &ArgMatches) {
    match matches.subcommand() {
        Some(("info", sub_m)) => { },
        Some(("erase", sub_m)) => { erase_db() },
        Some(("backup", sub_m)) => { },
        Some(("restore", sub_m)) => { }
        _ => { }
    }
}


fn process_new_command(arg_matches: &ArgMatches) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");

    let table: &String = arg_matches.get_one::<String>("table").expect("Missing required argument");
    db_connection.insert_new_into_table(table.clone()).expect("couldn't insert new row!");  

}

fn process_set_command(arg_matches: &ArgMatches) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");

    let table = arg_matches.get_one::<String>("table").expect("Missing required argument");
    let row_id = arg_matches.get_one::<i64>("row-id").expect("Missing required argument");
    let field = arg_matches.get_one::<String>("field").expect("Missing required argument");
    let value = arg_matches.get_one::<String>("value").expect("Missing required argument");

    db_connection.set_field_in_table(table.clone(), row_id.clone(), field.clone(), value.clone()).expect("couldn't set field!");
}

fn process_list_command(arg_matches: &ArgMatches) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection!");

    let table = arg_matches.get_one::<String>("table").expect("Missing required argument");

    let ids = db_connection.get_table_row_ids(table.clone()).expect("couldn't get table row ids");

    if ids.len() == 0 {
        println!("No entries in table {}", table);
    } else {
        for id in ids {
            println!("{}", id);
        }
    }

}

fn process_remove_command(arg_matches: &ArgMatches) {
    let db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection!");

    let table = arg_matches.get_one::<String>("table").expect("Missing required argument");
    let row_id = arg_matches.get_one::<i64>("row-id").expect("Missing required argument");

    db_connection.remove_row_in_table(table.clone(), RowId(*row_id)).expect("Couldn't remove row from table");
}
