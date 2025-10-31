use std::{env, path::PathBuf};
use clap::{Parser, Subcommand, Command, ArgMatches, Arg};
use framework::prelude::*;
use billing::InvoicePlugin;

fn main() {
    let mut app = App::new();
    app.add_plugin(DbPlugin::default())
        .add_plugin(InvoicePlugin::default());

    let mut command_args = std::env::args().collect::<Vec<_>>();
    // remove the initial "tacl"
    command_args.remove(0);
    app.execute(shlex::join(command_args.iter().map(|e| e.as_str())).as_str());
}

