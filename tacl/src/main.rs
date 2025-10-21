use std::{env, path::PathBuf};
use clap::{Parser, Subcommand, Command, ArgMatches, Arg};
use db::{DbPlugin, DatabaseConnection, RowId};
use framework::{App, Plugin};
use billing::InvoicePlugin;

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


