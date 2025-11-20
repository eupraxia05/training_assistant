//! A plugin that adds a set of commands for editing the database.
use framework::prelude::*;
use clap::{Command, Arg, ArgMatches};
use tui::{Tui, Tab};

#[derive(Clone)]
pub struct DbCommandsPlugin;

impl Plugin for DbCommandsPlugin {
    fn build(self, context: &mut Context) {        
        context.add_command(
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
    }
}

fn process_edit_command(
    context: &mut Context, 
    matches: &ArgMatches
) -> Result<CommandResponse> {
    context.add_resource(Tui::default().with_tabs([Tab::new("tab 1"), Tab::new("tab 2")]));
    Ok(CommandResponse::new("Starting TUI session..."))
}
