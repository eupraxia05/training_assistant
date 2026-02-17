//! A plugin that adds a set of commands for editing the database.
use clap::{Arg, ArgMatches, Command};
use dolmen::prelude::*;
use framework::prelude::*;
use gui::prelude::*;
use ratatui::crossterm::event::{
    KeyCode, KeyModifiers,
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{
        Block, HighlightSpacing, List, ListState,
        Paragraph, Row, StatefulWidget, Table,
        TableState, Widget, Wrap,
    },
};
use tabled::builder::Builder as TabledBuilder;
use tui::prelude::*;
use tui_textarea::Input;

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
    fn build(
        self,
        context: &mut Context,
    ) -> dolmen::Result<()> {
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
            )?
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
            )?
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
            )?
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
            )?
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
            )?;

        if context.has_resource::<TuiNewTabTypes>() {
            context.get_resource_mut::<TuiNewTabTypes>().unwrap().register_new_tab_type::<DbInfoTabImpl>("Database Info");
            context
                .get_resource_mut::<TuiNewTabTypes>()
                .unwrap()
                .register_new_tab_type::<EditTabImpl>(
                    "Edit Table",
                );
        }
        context
            .add_new_window_type::<TableEditorWindow>(
                "Table Editor",
            );
        Ok(())
    }
}

fn process_new_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> dolmen::Result<CommandResponse> {
    let db_connection = context.db_connection()?;
    let table: &String = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");
    let new_row_id = db_connection
        .new_row_in_table(table.clone())
        .expect("couldn't insert new row!");

    Ok(CommandResponse::new(format!(
        "Inserted new row (id: {}) in table {}.",
        new_row_id, table
    )))
}

fn process_set_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> dolmen::Result<CommandResponse> {
    let db_connection = context.db_connection()?;
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
) -> dolmen::Result<CommandResponse> {
    let db_connection = context.db_connection()?;
    let table = arg_matches
        .get_one::<String>("table")
        .expect("Missing required argument");

    let ids = db_connection
        .get_table_row_ids(table.clone())
        .expect("couldn't get table row ids");

    let response_text = if ids.is_empty() {
        format!("No entries in table {}.", table)
    } else {
        let Some(table_config) = db_connection
            .tables()
            .iter()
            .find(|t| t.table_name == *table)
        else {
            return Err(dolmen::Error::new(format!(
                "table does not exist: {}",
                table
            )));
        };

        let mut tabled_builder =
            TabledBuilder::default();
        (table_config.push_tabled_header_fn)(
            &mut tabled_builder,
        );
        for id in ids {
            (table_config.push_tabled_record_fn)(
                &mut tabled_builder,
                db_connection,
                table.to_string(),
                RowId(id),
            )
        }
        tabled_builder.build().to_string()
    };
    Ok(CommandResponse::new(response_text))
}

fn process_remove_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> dolmen::Result<CommandResponse> {
    let db_connection = context.db_connection()?;
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
) -> dolmen::Result<CommandResponse> {
    match matches.subcommand() {
        Some(("info", _)) => {
            let db_connection =
                context.db_connection()?;
            process_db_info_command(db_connection)
        }
        Some(("erase", _)) => {
            let db_connection =
                context.db_connection()?;
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

fn db_info_text(
    db_connection: &mut DbConnection,
) -> String {
    let mut response_text = String::default();
    if db_connection.is_open() {
        response_text += "Database connection open.\n";
        if let Some(db_path) = db_connection.db_path()
        {
            response_text += format!(
                "Database path: {:?}",
                db_path
            )
            .as_str();
        } else {
            response_text += "No database path (in-memory connection)";
        }
    } else {
        response_text +=
            "No database connection open.";
    }
    response_text
}

fn process_db_info_command(
    db_connection: &mut DbConnection,
) -> dolmen::Result<CommandResponse> {
    let response_text = db_info_text(db_connection);

    Ok(CommandResponse::new(response_text))
}

fn erase_db(
    db_connection: &mut DbConnection,
) -> dolmen::Result<CommandResponse> {
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

    fn render(
        context: &mut Context,
        buffer: &mut Buffer,
        rect: Rect,
        block: Block,
        _: usize,
    ) {
        // TODO: remove this unwrap()
        let db_connection = context
            .get_resource_mut::<DbConnection>()
            .unwrap();
        let text = db_info_text(db_connection);

        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true })
            .render(rect, buffer);
    }

    fn keybinds() -> Vec<KeyBind> {
        Vec::new()
    }

    fn handle_key(
        _context: &mut Context,
        _bind_name: &str,
        _tab_idx: usize,
    ) {
    }

    fn handle_text(
        _: &mut Context,
        _: ratatui::crossterm::event::Event,
        _: usize,
    ) {
    }
}

struct EditTabImpl;

#[derive(Default)]
struct EditTabState {
    list_state: ListState,
    table_name: Option<String>,
    available_tables: Vec<String>,
    table_state: Option<TableState>,
    edit_cell: Option<(usize, usize)>,
    display_err: Option<String>,
    text_area: Option<tui_textarea::TextArea<'static>>,
    edit_field_name: Option<String>,
}

impl TabImpl for EditTabImpl {
    type State = EditTabState;

    fn title() -> String {
        "✏️ Edit Table".into()
    }

    fn render(
        context: &mut Context,
        buffer: &mut Buffer,
        rect: Rect,
        block: Block,
        tab_id: usize,
    ) {
        let is_selecting_table = {
            // TODO: remove this unwrap
            let state = context.get_resource_mut::<TabState<EditTabState>>().unwrap().get_state_mut(tab_id).unwrap();
            state.table_name.is_none()
        };

        if is_selecting_table {
            // TODO: remove this unwrap
            let table_names = {
                let db_connection = context
                    .get_resource_mut::<DbConnection>()
                    .unwrap();
                db_connection
                    .tables()
                    .iter()
                    .map(|t| t.table_name.clone())
                    .collect::<Vec<_>>()
            };

            // TODO: get rid of these unwraps
            context.get_resource_mut::<TabState<EditTabState>>().unwrap().get_state_mut(tab_id).unwrap().available_tables = table_names.clone();

            if table_names.len() > 0 {
                let list = List::new(table_names)
                    .block(block)
                    .highlight_style(
                        Style::new()
                            .fg(Color::Black)
                            .bg(Color::White),
                    )
                    .highlight_symbol(">")
                    .highlight_spacing(
                        HighlightSpacing::Always,
                    );

                // TODO: remove these unwraps
                let state = context.get_resource_mut::<TabState<EditTabState>>().unwrap().get_state_mut(tab_id).unwrap();
                StatefulWidget::render(
                    list,
                    rect,
                    buffer,
                    &mut state.list_state,
                );
            } else {
                Paragraph::new("No tables.")
                    .block(block)
                    .render(rect, buffer);
            }
        } else {
            render_table_view(
                context, tab_id, block, rect, buffer,
            )
            .expect("failed to render table view");
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
                name: "move_right".into(),
                display_key: "Right".into(),
                display_name: "Move Right".into(),
                key_code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                name: "move_left".into(),
                display_key: "Left".into(),
                display_name: "Move Left".into(),
                key_code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                name: "select".into(),
                display_key: "Enter".into(),
                display_name: "Select".into(),
                key_code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                name: "back".into(),
                display_key: "Esc".into(),
                display_name: "Back".into(),
                key_code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            },
            KeyBind {
                name: "new_row".into(),
                display_key: "Ctrl+N".into(),
                display_name: "New Row".into(),
                key_code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyBind {
                name: "delete_row".into(),
                display_key: "Ctrl+D".into(),
                display_name: "Delete Row".into(),
                key_code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
            },
        ]
    }

    fn handle_key(
        context: &mut Context,
        bind: &str,
        tab_id: usize,
    ) {
        // TODO: get rid of these unwraps
        match bind {
            "move_up" => {
                let state = context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap();
                state.display_err = None;
                if let Some(table_state) =
                    &mut state.table_state
                {
                    table_state.select_previous();
                } else {
                    state.list_state.select_previous();
                }
            }
            "move_down" => {
                let state = context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap();
                state.display_err = None;
                if let Some(table_state) =
                    &mut state.table_state
                {
                    table_state.select_next();
                } else {
                    state.list_state.select_next();
                }
            }
            "move_right" => {
                let state = context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap();
                state.display_err = None;
                if let Some(table_state) =
                    &mut state.table_state
                {
                    table_state.select_next_column();
                }
            }
            "move_left" => {
                let state = context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap();
                state.display_err = None;
                if let Some(table_state) =
                    &mut state.table_state
                {
                    table_state
                        .select_previous_column();
                }
            }
            "select" => {
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .display_err = None;
                let is_editing_table = context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .table_name
                    .is_some();
                if is_editing_table {
                    if let Err(e) =
                        on_select_cell(context, tab_id)
                    {
                        context.tab_state_mut::<EditTabState>(tab_id).unwrap().display_err = Some(e.message().clone().unwrap_or("".into()));
                    }
                } else {
                    let state = context
                        .tab_state_mut::<EditTabState>(
                            tab_id,
                        )
                        .unwrap();
                    // TODO: get rid of this unwrap
                    // TODO: range check
                    let table_name = state
                        .available_tables[state
                        .list_state
                        .selected()
                        .unwrap()]
                    .clone();
                    state.table_name =
                        Some(table_name);
                    let mut table_state =
                        TableState::default();
                    table_state
                        .select_cell(Some((0, 0)));
                    state.table_state =
                        Some(table_state);
                }
            }
            "back" => {
                let state = context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap();
                state.display_err = None;
                if state.edit_cell.is_some() {
                    state.edit_cell = None;
                    state.text_area = None;
                } else if state.table_name.is_some() {
                    state.table_name = None;
                    state.table_state = None;
                }
            }
            "new_row" => {
                let table_name = context
                    .tab_state::<EditTabState>(tab_id)
                    .unwrap()
                    .table_name
                    .clone()
                    .unwrap();
                context
                    .db_connection()
                    .unwrap()
                    .new_row_in_table(table_name);
            }
            "delete_row" => {
                let tab_state = context
                    .tab_state::<EditTabState>(tab_id)
                    .unwrap();
                if tab_state.table_name.is_some()
                    && tab_state.table_state.is_some()
                {
                    let table_name = tab_state
                        .table_name
                        .as_ref()
                        .unwrap()
                        .clone();
                    let row_id = tab_state
                        .table_state
                        .as_ref()
                        .unwrap()
                        .selected()
                        .unwrap()
                        + 1;

                    context
                        .db_connection()
                        .unwrap()
                        .remove_row_in_table(
                            table_name,
                            RowId(row_id as i64),
                        );
                }
            }
            _ => {}
        }
    }

    fn handle_text(
        context: &mut Context,
        ev: ratatui::crossterm::event::Event,
        tab_id: usize,
    ) {
        match ev.clone().into() {
            Input {
                key: tui_textarea::Key::Esc,
                ..
            } => {
                context
                    .get_resource_mut::<Tui>()
                    .unwrap()
                    .set_input_mode(
                        tui::TuiInputMode::Bind,
                    );
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .text_area = None;
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .edit_field_name = None;
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .edit_cell = None;
            }
            Input {
                key: tui_textarea::Key::Enter,
                ..
            } => {
                println!("enter detected");
                let state = context
                    .tab_state::<EditTabState>(tab_id)
                    .unwrap();
                let table_name =
                    state.table_name.clone().unwrap();
                let edit_cell =
                    state.edit_cell.unwrap();
                let edit_row = edit_cell.0;
                let field_name = state
                    .edit_field_name
                    .clone()
                    .unwrap();
                let text = state
                    .text_area
                    .clone()
                    .unwrap()
                    .into_lines()[0]
                    .clone();
                context
                    .db_connection()
                    .unwrap()
                    .set_field_in_table(
                        table_name,
                        RowId((edit_row + 1) as i64),
                        field_name,
                        text,
                    );
                context
                    .get_resource_mut::<Tui>()
                    .unwrap()
                    .set_input_mode(
                        tui::TuiInputMode::Bind,
                    );
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .text_area = None;
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .edit_field_name = None;
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .edit_cell = None;
            }
            input => {
                context
                    .tab_state_mut::<EditTabState>(
                        tab_id,
                    )
                    .unwrap()
                    .text_area
                    .as_mut()
                    .unwrap()
                    .input(input);
            }
        }
    }
}

fn on_select_cell(
    context: &mut Context,
    tab_id: usize,
) -> dolmen::Result<()> {
    let table_name = context
        .tab_state::<EditTabState>(tab_id)?
        .table_name
        .clone()
        .ok_or(dolmen::Error::new(
            "not editing table",
        ))?;
    let table_config = context
        .db_connection()?
        .tables()
        .iter()
        .find(|t| t.table_name == table_name)
        .ok_or(dolmen::Error::new(
            "couldn't find table config",
        ))?;
    let field_types = (table_config.field_types_fn)();
    let selected_cell = context
        .tab_state::<EditTabState>(tab_id)?
        .table_state
        .as_ref()
        .ok_or(dolmen::Error::new(
            "couldn't get table state",
        ))?
        .selected_cell()
        .ok_or(dolmen::Error::new(
            "couldn't get selected cell",
        ))?;
    let field_type = field_types.get(selected_cell.1).ok_or(dolmen::Error::new(format!("couldn't get field type id, field_types: {:?}", field_types)))?;

    let field_type_id = field_type.type_id;
    context
        .tab_state_mut::<EditTabState>(tab_id)
        .unwrap()
        .edit_field_name =
        Some(field_type.name.clone());
    if field_type_id
        != std::any::TypeId::of::<String>()
    {
        return Err(dolmen::Error::new(
            "can't edit field type",
        ));
    }
    if let Some(table_state) = &mut context
        .tab_state_mut::<EditTabState>(tab_id)?
        .table_state
    {
        context
            .tab_state_mut::<EditTabState>(tab_id)?
            .edit_cell = table_state.selected_cell();
        context
            .tab_state_mut::<EditTabState>(tab_id)?
            .text_area =
            Some(tui_textarea::TextArea::default());
        context
            .get_resource_mut::<Tui>()
            .ok_or(dolmen::Error::default())?
            .set_input_mode(tui::TuiInputMode::Text);
    }
    Ok(())
}

fn render_table_view(
    context: &mut Context,
    tab_id: usize,
    block: Block,
    rect: Rect,
    buffer: &mut Buffer,
) -> dolmen::Result<()> {
    // TODO: dejank this
    let table_name = context
        .tab_state::<EditTabState>(tab_id)?
        .table_name
        .clone()
        .unwrap();
    // TODO: get rid of this unwrap
    let db_connection = context
        .get_resource_mut::<DbConnection>()
        .unwrap();
    let table_config = db_connection
        .tables()
        .iter()
        .find(|t| t.table_name == table_name)
        .unwrap();
    let field_names = (table_config.field_names_fn)();

    // TODO: remove this unwrap
    let row_ids = db_connection
        .get_table_row_ids(table_name.clone())
        .unwrap();

    let mut rows = Vec::new();

    for row in &row_ids {
        rows.push(Row::new((table_config
            .get_fields_as_strings_fn)(
            db_connection,
            table_name.clone(),
            RowId(*row),
        )));
    }

    let widths = field_names.iter().map(|f| {
        Constraint::Min(f.len().try_into().unwrap())
    });

    let table_block = Block::new()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(
            ratatui::widgets::BorderType::Rounded,
        );

    let table = Table::new(rows, widths)
        .column_spacing(1)
        .header(
            Row::new(field_names)
                .style(Style::new().reversed()),
        )
        .footer(Row::new(vec![format!(
            "{} rows",
            row_ids.len()
        )]))
        .block(table_block.clone())
        .cell_highlight_style(Style::new().reversed())
        .highlight_symbol(">>");

    let tab_state = context
        .tab_state_mut::<EditTabState>(tab_id)?;

    let layout = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(3),
    ]);

    Widget::render(block.clone(), rect, buffer);

    let [table_area, edit_field_area] =
        layout.areas(block.inner(rect));

    if let Some(table_state) =
        &mut tab_state.table_state
    {
        StatefulWidget::render(
            table,
            table_area,
            buffer,
            table_state,
        );
    } else {
        Widget::render(table, table_area, buffer);
    }

    let edit_field_block = Block::new()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(
            ratatui::widgets::BorderType::Rounded,
        );

    if let Some(display_err) =
        tab_state.display_err.as_ref()
    {
        let err_text =
            Paragraph::new(Line::raw(display_err))
                .block(edit_field_block);
        Widget::render(
            err_text,
            edit_field_area,
            buffer,
        );
    } else {
        if let Some(text_area) =
            &mut tab_state.text_area
        {
            //text_area.set_block(block.clone());
            text_area.render(edit_field_area, buffer);
        } else {
            let edit_field =
                Paragraph::new(Line::raw(""))
                    .block(edit_field_block);
            Widget::render(
                edit_field,
                edit_field_area,
                buffer,
            );
        }
    }

    Ok(())
}

struct TableEditorWindow;
