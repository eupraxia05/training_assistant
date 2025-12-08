//! A plugin that adds a set of commands for editing the database.
use framework::prelude::*;
use clap::{Command, Arg, ArgMatches};
use tui::{KeyBind, TabImpl, TuiNewTabTypes, TabState};
use ratatui::{
    widgets::{Wrap, Block, Paragraph, Widget, Row, Table, List, ListState, HighlightSpacing, StatefulWidget},
    style::{Style, Stylize, Color},
    buffer::Buffer,
    layout::{Constraint, Rect},
};
use crossterm::event::{KeyModifiers, KeyCode};
use tabled::{builder::Builder as TabledBuilder};

///////////////////////////////////////////////////////////////////////////////
// PUBLIC API
///////////////////////////////////////////////////////////////////////////////

/// The plugin for database commands. Add this to set up the required
/// commands.
#[derive(Clone)]
pub struct DbCommandsPlugin;

///////////////////////////////////////////////////////////////////////////////
// PRIVATE IMPLEMENTATION
///////////////////////////////////////////////////////////////////////////////
impl Plugin for DbCommandsPlugin {
    fn build(self, context: &mut Context) {        
        context
            .add_command(Command::new("new")
                .about("Add a new row to a table")
                .arg(
                    Arg::new("table")
                        .long("table")
                        .required(true)
                        .help("Name of the table to add a row in")
                ),
                process_new_command
            )
            .add_command(
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
            )
            .add_command(
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
            )
            .add_command(
                Command::new("list").alias("ls")
                    .about("Lists the rows of a table")
                    .arg(
                        Arg::new("table")
                            .long("table")
                            .required(true)
                            .help("Name of the table to list rows from")
                    ),
                process_list_command
            )
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
            );

        context.get_resource_mut::<TuiNewTabTypes>().unwrap().register_new_tab_type::<DbInfoTabImpl>("Database Info");
        context.get_resource_mut::<TuiNewTabTypes>().unwrap().register_new_tab_type::<EditTabImpl>("Edit Table");
    }
}

fn process_new_command(
    context: &mut Context,
    arg_matches: &ArgMatches, 
) -> Result<CommandResponse> {
    let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;
    let table: &String = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");
    let new_row_id = db_connection
        .new_row_in_table(table.clone())
        .expect("couldn't insert new row!");

    Ok(CommandResponse::new(
        format!("Inserted new row (id: {}) in table {}.", new_row_id, table)
    ))
}

fn process_set_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> Result<CommandResponse> {
    let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;
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
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> Result<CommandResponse> {
    let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;
    let table = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");

    let ids = db_connection
        .get_table_row_ids(table.clone())
        .expect("couldn't get table row ids");

    let response_text = if ids.is_empty() {
        format!("No entries in table {}.", table)
    } else {
        let Some(table_config) = db_connection.tables().iter().find(|t| t.table_name == *table) else {
            return Err(Error::UnknownError);
        };

        let mut tabled_builder = TabledBuilder::default();
        (table_config.push_tabled_header_fn)(&mut tabled_builder);
        for id in ids {
            (table_config.push_tabled_record_fn)(&mut tabled_builder, db_connection, table.to_string(), RowId(id))
        }
        tabled_builder.build().to_string()
    };
    Ok(CommandResponse::new(response_text))
}

fn process_remove_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> Result<CommandResponse> {
    let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;
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

fn process_db_command(
    context: &mut Context,
    matches: &ArgMatches,
) -> Result<CommandResponse> {
    match matches.subcommand() {
        Some(("info", _)) => {
            let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;
            process_db_info_command(db_connection)
        }
        Some(("erase", _)) => {
            let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;
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

struct DbInfoTabImpl;

#[derive(Default)]
struct DbInfoTabState;

impl TabImpl for DbInfoTabImpl {
    type State = DbInfoTabState;

    fn title() -> String {
        "Database Info".into()
    }
    
    fn render(context: &mut Context, buffer: &mut Buffer,
        rect: Rect, block: Block, _: usize
    ) {
        // TODO: remove this unwrap()
        let db_connection = context.get_resource_mut::<DbConnection>().unwrap();
        let text = db_info_text(db_connection);

        Paragraph::new(text).block(block).wrap(Wrap { trim: true }).render(rect, buffer);
    }

    fn keybinds() -> Vec<KeyBind> {
        Vec::new()
    }

    fn handle_key(_context: &mut Context, _bind_name: &str, _tab_idx: usize) {

    }
}

struct EditTabImpl;

#[derive(Default)]
struct EditTabState {
    list_state: ListState,
    table_name: Option<String>,
    available_tables: Vec<String>,
}

impl TabImpl for EditTabImpl {
    type State = EditTabState;

    fn title() -> String {
        "✏️ Edit Table".into()
    }

    fn render(context: &mut Context, buffer: &mut Buffer,
        rect: Rect, block: Block, tab_id: usize
    ) {
        let is_selecting_table = {
            // TODO: remove this unwrap
            let state = context.get_resource_mut::<TabState<EditTabState>>().unwrap().get_state_mut(tab_id).unwrap();
            state.table_name.is_none()
        };

        if is_selecting_table {
            // TODO: remove this unwrap
            let table_names = {
                let db_connection = context.get_resource_mut::<DbConnection>().unwrap(); 
                db_connection.tables().iter().map(|t| t.table_name.clone()).collect::<Vec<_>>()
            };

            // TODO: get rid of these unwraps
            context.get_resource_mut::<TabState<EditTabState>>().unwrap().get_state_mut(tab_id).unwrap().available_tables = table_names.clone();

            if table_names.len() > 0 {
                let list = List::new(table_names)
                    .block(block)
                    .highlight_style(Style::new().fg(Color::Black).bg(Color::White))
                    .highlight_symbol(">")
                    .highlight_spacing(HighlightSpacing::Always);

                // TODO: remove these unwraps
                let state = context.get_resource_mut::<TabState<EditTabState>>().unwrap().get_state_mut(tab_id).unwrap();
                StatefulWidget::render(list, rect, buffer, &mut state.list_state); 
            } else {
                Paragraph::new("No tables.").block(block).render(rect, buffer);
            }
        } else {
            // TODO: dejank this
            let table_name = context.get_resource_mut::<TabState<EditTabState>>().unwrap().get_state_mut(tab_id).unwrap().table_name.clone().unwrap();
            // TODO: get rid of this unwrap
            let db_connection = context.get_resource_mut::<DbConnection>().unwrap();
            let table_config = db_connection.tables().iter().find(|t| t.table_name == table_name).unwrap();
            let field_names = (table_config.field_names_fn)();
           
            // TODO: remove this unwrap
            let row_ids = db_connection.get_table_row_ids(table_name.clone()).unwrap();
            
            let mut rows = Vec::new();

            for row in &row_ids {
                rows.push(Row::new((table_config.get_fields_as_strings_fn)(db_connection, table_name.clone(), RowId(*row))));
            }
            
            let widths = field_names.iter().map(|f| Constraint::Min(f.len().try_into().unwrap()));
            let table = Table::new(rows, widths)
                .column_spacing(1)
                .header(
                    Row::new(field_names)
                    .style(Style::new().bold())
                    .bottom_margin(1)
                )
                .footer(Row::new(vec![format!("{} rows", row_ids.len())]))
                .block(block.clone())
                .row_highlight_style(Style::new().reversed())
                .highlight_symbol(">>");
            Widget::render(table, rect, buffer);

        }
    }

    fn keybinds() -> Vec<KeyBind> {
        vec![
            KeyBind {
                name: "move_up".into(),
                display_key: "Up".into(),
                display_name: "Move Up".into(),
                key_code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                name: "move_down".into(),
                display_key: "Down".into(),
                display_name: "Move Down".into(),
                key_code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                name: "select".into(),
                display_key: "Enter".into(),
                display_name: "Select".into(),
                key_code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            }
        ]
    }
    
    fn handle_key(context: &mut Context, bind: &str, tab_id: usize) {
        // TODO: get rid of these unwraps
        let state = context.get_resource_mut::<TabState<EditTabState>>().unwrap()
            .get_state_mut(tab_id).unwrap();
        
        match bind {
            "move_up" => {
                state.list_state.select_previous();
            },
            "move_down" => {
                state.list_state.select_next();
            },
            "select" => {
                // TODO: get rid of this unwrap
                // TODO: range check
                let table_name = state.available_tables[state.list_state.selected().unwrap()].clone();
                state.table_name = Some(table_name);
            }
            _ => { }
        }
    }
}
