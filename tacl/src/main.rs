use std::{env, path::PathBuf};
use clap::{Parser, Subcommand, Command, ArgMatches, Arg};
use db::{DatabaseConnection, TrainerId, ClientId, RowId};
use framework::{App, Plugin};

#[derive(Default, Clone)]
struct DbPlugin;

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
   let mut conn =  DatabaseConnection::open_default().expect("Couldn't open database connection");
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

#[derive(Default, Clone)]
struct InvoicePlugin;

impl Plugin for InvoicePlugin {
    fn build(self, app: &mut App) {
        app.add_command(Command::new("invoice")
            .alias("inv")
            .about("Invoice related commands")
            .subcommand(Command::new("generate")
                .alias("gen")
                .about("Generates an invoice document")
                .arg(Arg::new("invoice-id")
                    .long("invoice-id")
                    .value_parser(clap::value_parser!(i64))
                    .required(true)
                    .help("The invoice row ID to generate a document from.")
                )
                .arg(Arg::new("trainer-id")
                    .long("trainer-id")
                    .value_parser(clap::value_parser!(i64))
                    .required(true)
                    .help("The trainer row ID.")
                )
                .arg(Arg::new("client-id")
                    .long("client-id")
                    .value_parser(clap::value_parser!(i64))
                    .required(true)
                    .help("The client row ID.")
                )
                .arg(Arg::new("out-folder")
                    .long("out-folder")
                    .value_parser(clap::value_parser!(PathBuf))
                    .required(true)
                    .help("The folder to output the document to")
                )
            ),
            process_invoice_command
        )
    }
}

fn main() {
    let app = App::new()
        .add_plugin(DbPlugin::default())
        .add_plugin(InvoicePlugin::default());

    let mut command = Command::new("tacl")
        .version("0.1.0")
        .about("Command line interface for Training Assistant")
        .subcommand_required(true);

    for (c, _) in app.commands() {
        command = command.subcommand(c);
    }

    let matches = command.get_matches();

    if let Some(subcommand_name) = matches.subcommand_name() {
        for (c, f) in app.commands() {
            if c.get_name() == subcommand_name {
                if let Some(subcommand_matches) = matches.subcommand_matches(subcommand_name) {
                   f(subcommand_matches); 
                }
            }
        }
    }
}

fn process_invoice_generate_command(arg_matches: &ArgMatches) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");

    let invoice_row_id = arg_matches.get_one::<i64>("invoice-id").expect("Missing required argument");
    let client_row_id = arg_matches.get_one::<i64>("client-id").expect("Missing required argument");
    let trainer_row_id = arg_matches.get_one::<i64>("trainer-id").expect("Missing required argument");
    let out_folder = arg_matches.get_one::<PathBuf>("out-folder").expect("Missing required argument");

    billing::create_invoice(&mut db_connection, out_folder.clone(), RowId(*invoice_row_id), RowId(*trainer_row_id), RowId(*client_row_id));
}

fn process_invoice_command(arg_matches: &ArgMatches) { 
    match arg_matches.subcommand() {
        Some(("generate", sub_m)) => {process_invoice_generate_command(sub_m)},
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
