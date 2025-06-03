use rusqlite::{Connection, Result};

// TODO: change path to some sane default value (possibly allow using a custom DB location later)
pub fn init_db() -> Result<()> {
    let path = "./exercise_db.db3";

    // TODO: move this out of this function to allow re-use for other db calls (this is thread safe with rusqlite via compile time checks)
    let conn = Connection::open(path)?;

    // transaction creates initial exercises table. additional tables (client/etc) should be added in this transaction
    // TODO: only do this if tables do not yet exist
    // TODO: handle error
    conn.execute_batch(
        "BEGIN;
        CREATE TABLE exercises(
            id          INT PRIMARY KEY,
            name        TEXT NOT NULL,
            summary     TEXT,
            steps       TEXT,
            image_paths TEXT);
        COMMIT;");
    Ok(())
} 