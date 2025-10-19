use uuid::Uuid;
use std::fs;
use std::result;
use rusqlite::{Connection, params, ToSql, types::FromSql};
use std::path::{Path, PathBuf};

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

    pub fn open_default() -> Result<Self> {
        let db_path = Self::get_default_db_path()?;
        Self::open_from_path(&db_path)
    }

    pub fn open_test() -> Result<Self> {
        let db_path = Self::get_test_db_path()?;
        Self::open_from_path(&db_path)
    }

    fn open_from_path(path: &PathBuf) -> Result<Self> {
        println!("opening database connection at {:?}", path);

        fs::create_dir_all(path.parent().unwrap()).map_err(|e| 
            Error::FileError(format!("failed to create dirs for path {:?}: {:?}", path, e.to_string()))
        )?;
        let mut connection = Connection::open(path.clone())
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        
        Client::setup(&mut connection)?;

        connection
            .execute_batch(
                "BEGIN;
                CREATE TABLE IF NOT EXISTS trainers(
                    id   INTEGER PRIMARY KEY,
                    name TEXT,
                    companyname TEXT,
                    address TEXT,
                    email TEXT,
                    phone TEXT);
                CREATE TABLE IF NOT EXISTS invoices(
                    id INTEGER PRIMARY KEY,
                    invoice_number TEXT,
                    due_date TEXT,
                    date_paid TEXT,
                    paid_via TEXT,
                    charges TEXT
                );
                CREATE TABLE IF NOT EXISTS charges(
                    id INTEGER PRIMARY KEY,
                    date TEXT,
                    description TEXT,
                    amount INTEGER
                );
                COMMIT;")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

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
    name: String
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

#[cfg(test)]
mod tests {
    use crate::{Result, Error, DatabaseConnection};
    use std::fs;
    
    #[test] 
    fn open_connection_test() -> Result<()> {
        let mut conn = DatabaseConnection::open_test()?;
        assert!(conn.is_open());
        let db_path = conn.db_path().clone();
        assert!(fs::exists(conn.db_path()).map_err(|e| Error::FileError(format!("failed to check if db exists: {:?}", e.to_string())))?);
        Ok(())
    }    
}
