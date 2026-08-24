use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use rusqlite::Connection;

pub fn open_connection(db_path: &Path) -> Connection {
    if !std::fs::exists(db_path)
        .expect("couldn't check if db exists")
    {
        println!("creating db path");
        std::fs::create_dir_all(
            db_path
                .parent()
                .expect("couldn't get db path parent"),
        )
        .expect("couldn't create db path");
    }

    let mut connection = Connection::open(db_path)
        .expect("Failed to open database connection");

    connection
        .execute(
            include_str!("connection_setup.sql"),
            [],
        )
        .expect("failed to run connection setup");

    connection
}

#[cfg(test)]
pub fn build_test_db_path() -> PathBuf {
    let dirs = ProjectDirs::from(
        "",
        "",
        "training_assistant_test",
    )
    .unwrap();

    dirs.data_dir().join("data.db").into()
}

#[cfg(test)]
mod test {
    use crate::db::*;

    #[test]
    fn connection_basics() {
        let connection =
            open_connection(&build_test_db_path());
    }
}
