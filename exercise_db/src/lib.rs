use rusqlite::{Connection, Result};
use std::path::{Path};

// TODO: consider making this a struct which stores (and returns) the db interface
// TODO: consider adding support for a human readable format (json, etc)
// TODO: spec out expected db interactions (search by name, add new exercise, get exercise list by name, get exercise list by id, (low-pri) editing fields on a given exercise etc)
// TODO: add schema version value to table, plan for migrations from old formats (upgrading should never break custom/modified exercises)
// TODO: convert to use a service object owned by caller rather than creating new connection in every function
// TODO: add tests
pub fn init_db(data_path: &Path) -> Result<()> {
    let db_path = data_path.join("exercises_db.db3");
    let conn = Connection::open(db_path)?;

    // transaction creates initial exercises table. additional tables (client/etc) should be added in this transaction
    // TODO: handle error
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE IF NOT EXISTS exercises(
            id          INT PRIMARY KEY,
            name        TEXT NOT NULL,
            summary     TEXT,
            steps       TEXT,
            image_paths TEXT);
        COMMIT;");
    Ok(())
}

pub fn search_exercises_by_name(ex_name: &str) {}

pub fn add_exercise(new_exercise: Exercise) {}

// TODO: confirm if array of strings is a coherent way to pass this
// TODO: look into representing this and the following function as 2 different overloads of get_exercises instead
pub fn get_exercises_by_name(ex_name: &[&str]) {}

pub fn get_exercises_by_id(ex_id: &[i32]) {}

// TODO: look at ways to detect at compile time (or at least test time) when this falls out of sync with db table format
#[derive(Debug)]
pub struct Exercise {
    db_id: i32,
    name: String,
    summary: String,
    steps: String,
    image_paths: String
}
