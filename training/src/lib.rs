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
}

impl DatabaseConnection {
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
        let connection = Connection::open(path.clone())
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        
        connection
            .execute_batch(
                "BEGIN;
                CREATE TABLE IF NOT EXISTS clients(
                    id   INTEGER PRIMARY KEY,
                    name TEXT);
                COMMIT;")
            .map_err(|e| Error::DatabaseError(e.to_string()))?;

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

    pub fn add_client<S>(&mut self, name: S) -> Result<ClientId>
        where S: Into<String>
    {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };
        connection.execute_batch(format!(
            "BEGIN;
            INSERT INTO clients (name)
            VALUES
                (\"{}\");
            COMMIT;",
            name.into()
        ).as_str())?;
        Ok(ClientId(connection.last_insert_rowid()))
    }

    pub fn clients(&mut self) -> Result<Vec<ClientMetadata>> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };
        let mut select = connection.prepare("SELECT id, name FROM clients")?;
    
        Ok(select.query_map([], |row| {
            Ok(ClientMetadata {
                id: ClientId(row.get(0)?),
                name: row.get(1)?
            })
        })?.filter_map(|c| {c.ok()}).collect())
    }

    pub fn remove_client(&mut self, id: ClientId) -> Result<()> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(
            "DELETE FROM clients
            WHERE id = ?",
            [id.0])?;
        Ok(())
    }

    pub fn get_client_metadata(&self, id: ClientId) -> Result<ClientMetadata> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        let mut select = connection.prepare("SELECT name FROM clients WHERE id = ?")?;

        select.query_one([id.0], |t| {
            Ok(ClientMetadata {
                id,
                name: t.get(0)?,
            })
        }).map_err(|e| Error::DatabaseError(e.to_string()))
    }

    pub fn add_trainer<S>(&mut self, name: S) -> Result<TrainerId>
        where S: Into<String>
    {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };
        connection.execute_batch(format!(
            "BEGIN;
            INSERT INTO trainers (name)
            VALUES
                (\"{}\");
            COMMIT;",
            name.into()
        ).as_str())?;
        Ok(TrainerId(connection.last_insert_rowid()))
    }

    pub fn trainers(&self) -> Result<Vec<TrainerMetadata>> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };
        
        let mut select = connection.prepare("SELECT id, name, companyname, address, email, phone FROM trainers")?;
    
        Ok(select.query_map([], |row| {
            Ok(TrainerMetadata {
                id: TrainerId(row.get(0)?),
                name: row.get(1)?,
                company_name: row.get(2).unwrap_or_default(),
                address: row.get(3).unwrap_or_default(),
                email: row.get(4).unwrap_or_default(),
                phone: row.get(5).unwrap_or_default(),
            })
        })?.filter_map(|c| {c.ok()}).collect())
    }

    pub fn remove_trainer(&mut self, id: TrainerId) -> Result<()> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(
            "DELETE FROM trainers
            WHERE id = ?",
            [id.0])?;
        Ok(())
    }


    pub fn get_trainer_metadata(&self, id: TrainerId) -> Result<TrainerMetadata> {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        let mut select = connection.prepare("SELECT name, companyname, address, email, phone FROM trainers WHERE id = ?")?;

        select.query_one([id.0], |t| {
            Ok(TrainerMetadata {
                id,
                name: t.get(0)?,
                company_name: t.get(1).unwrap_or_default(),
                address: t.get(2).unwrap_or_default(),
                email: t.get(3).unwrap_or_default(),
                phone: t.get(4).unwrap_or_default()
            })
        }).map_err(|e| Error::DatabaseError(e.to_string()))
    }
    pub fn set_trainer_metadata_field<V>(
        &self, 
        id: TrainerId, 
        field: String, 
        value: V
    ) -> Result<()>
        where V: ToSql
    {
        let Some(connection) = &self.connection else {
            return Err(Error::NoConnectionError);
        };

        connection.execute(format!("UPDATE trainers
            SET {} = ?1
        WHERE
        id = ?2", field).as_str(), params![value, id.0])?;

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
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RowId(pub i64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ClientId(pub i64);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TrainerId(pub i64);

pub struct ClientMetadata {
    id: ClientId,
    name: String
}

impl ClientMetadata {
    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}

#[derive(Debug)]
pub struct TrainerMetadata {
    id: TrainerId,
    name: String,
    company_name: String,
    address: String,
    email: String,
    phone: String,
}

impl TrainerMetadata {
    pub fn id(&self) -> TrainerId {
        self.id
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn company_name(&self) -> &String {
        &self.company_name
    }

    pub fn address(&self) -> &String {
        &self.address
    }

    pub fn email(&self) -> &String {
        &self.email
    }

    pub fn phone(&self) -> &String {
        &self.phone
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
    
    #[test]
    fn add_remove_client() -> Result<()> {
        let mut conn = DatabaseConnection::open_test()?;
        let client_id = conn.add_client("Clarissa")?;
        assert!(conn.clients()?.iter().any(|c| c.id == client_id));
        conn.remove_client(client_id)?;
        assert!(!conn.clients()?.iter().any(|c| c.id == client_id));
        Ok(())
    }
}

/*pub fn init_clients_db() -> Result<()> {
    let conn = open_clients_db_connection().expect("Couldn't open connection to clients db");

    Ok(())
}

pub fn add_client(name: &'static str) -> Result<()> {
   let conn = open_clients_db_connection().expect("Couldn't open connection to clients db");
   let batch = format!(
        "BEGIN;
        INSERT INTO clients (name)
        VALUES
            (\"{}\");
        COMMIT;",
        name
    );
   conn.execute_batch(batch.as_str())?;
   Ok(())
}

fn open_clients_db_connection() -> Result<Connection> {
}

pub struct ClientMetadata {
    id: u32,
    name: String
}

impl ClientMetadata {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &String {
        &self.name
    }
}

pub fn get_client_metadata() -> Result<Vec<ClientMetadata>> {
    let conn = open_clients_db_connection()?;

    Ok(result)
}

#[cfg(test)]
mod test
{
    use crate::{init_clients_db, add_client, get_client_metadata};
    use rusqlite::Result;

    #[test]
    fn test_init_clients_db() -> Result<()> {
        init_clients_db()?;
        Ok(())
    }

    #[test]
    fn test_add_client() -> Result<()> {
        init_clients_db()?;
        add_client("Clarissa")?;
        let clients = get_client_metadata()?;
        assert_eq!(clients.len(), 1);
        for client in clients {
            println!("id: {}", client.id());
            println!("name: {}", client.name());
        }
        Ok(())
    }
}*/
