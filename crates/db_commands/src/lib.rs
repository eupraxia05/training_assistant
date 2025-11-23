//! A plugin that adds a set of commands for editing the database.
use framework::prelude::*;
use clap::{Command, Arg, ArgMatches};
use tui::{Tui, KeyBind, Tab, TabImpl, TuiNewTabTypes};
use ratatui::{
    widgets::{Wrap, Block, Paragraph, Widget},
    text::Line,
    style::Stylize,
    buffer::Buffer,
    layout::Rect,
};

#[derive(Clone)]
pub struct DbCommandsPlugin;

impl Plugin for DbCommandsPlugin {
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
                process_db_command
            )
            .add_command(
            Command::new("edit")
                .about("Edits a table row in TUI mode.")
                .arg(
                    Arg::new("table")
                        .long("table")
                        .required(true)
                        .help("Name of the table to edit")
                ),
            process_edit_command
        );

        context.get_resource_mut::<TuiNewTabTypes>().unwrap().register_new_tab_type::<DbInfoTabImpl>("Database Info");
        context.get_resource_mut::<TuiNewTabTypes>().unwrap().register_new_tab_type::<EditTabImpl>("Edit Table");
    }
}

fn process_db_command(
    context: &mut Context,
    matches: &ArgMatches,
) -> Result<CommandResponse> {
    match matches.subcommand() {
        Some(("info", _)) => {
            let db_connection = context.db_connection().unwrap();
            process_db_info_command(db_connection)
        }
        Some(("erase", _)) => {
            let db_connection = context.db_connection().unwrap();
            erase_db(db_connection)
        }
        Some(("backup", _)) => {
            Ok(CommandResponse::default())
        }
        Some(("restore", _)) => {
            Ok(CommandResponse::default())
        }
        _ => Ok(CommandResponse::default()),
    }
}

fn db_info_text(db_connection: &mut DbConnection) -> String {
    let mut response_text = String::default();
    if db_connection.is_open() {
        response_text += "Database connection open.\n";
        if let Some(db_path) = db_connection.db_path() {
            response_text += format!("Database path: {:?}", db_path).as_str();
        } else {
            response_text += "No database path (in-memory connection)";
        }
    } else {
        response_text += "No database connection open.";
    }
    response_text
}

fn process_db_info_command(db_connection: &mut DbConnection) -> Result<CommandResponse> {
    let response_text = db_info_text(db_connection); 

    Ok(CommandResponse::new(response_text))
}

fn erase_db(
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
    db_connection.delete_db()?;
    Ok(CommandResponse::default())
}

fn process_edit_command(
    context: &mut Context, 
    matches: &ArgMatches
) -> Result<CommandResponse> {
    context.add_resource(Tui::default().with_tabs(
        [Tab::new::<EditTabImpl>("tab 1"), Tab::new::<EditTabImpl>("tab 2")])
    );
    Ok(CommandResponse::new("Starting TUI session..."))
}

struct DbInfoTabImpl;

impl TabImpl for DbInfoTabImpl {
    fn title() -> String {
        "Database Info".into()
    }
    
    fn render(context: &mut Context, buffer: &mut Buffer,
        rect: Rect, block: Block
    ) {
        let db_connection = context.db_connection().unwrap();
        let text = db_info_text(db_connection);

        Paragraph::new(text).block(block).wrap(Wrap { trim: true }).render(rect, buffer);
    }

    fn keybinds() -> Vec<KeyBind> {
        Vec::new()
    }

    fn handle_key(context: &mut Context, bind_name: &str, tab_idx: usize) {

    }
}

struct EditTabImpl;

impl TabImpl for EditTabImpl {
    fn title() -> String {
        "Edit Tab".into()
    }

    fn render(context: &mut Context, buffer: &mut Buffer,
        rect: Rect, block: Block
    ) {
        Paragraph::new("edit tab content").block(block).render(rect, buffer);     
    }

    fn keybinds() -> Vec<KeyBind> {
        Vec::new()
    }
    
    fn handle_key(context: &mut Context, bind_name: &str, tab_idx: usize) {

    }
}
