use std::{env, path::PathBuf};
use clap::{Parser, Subcommand};

#[derive(Subcommand)] 
enum Commands {
  Handoutgen {
    #[clap(short = 'i', long, env)]
    handout_file: PathBuf,

    #[clap(short = 'o', long, env)]
    out_file: PathBuf
  },
  Inithandout {
    #[clap(short = 'o', long, env)]
    out_file: PathBuf
  }
}

#[derive(Parser)]
struct CommandlineArgs {
  #[command(subcommand)]
  command: Commands
}

use handoutgen::{generate_document, init_handout};

fn main() {
  let commandline_args = CommandlineArgs::parse();

  match commandline_args.command {
    Commands::Handoutgen { handout_file, out_file } => {
      match generate_document(handout_file, out_file) {
        Err(e) => {
          println!("Error generating document: {:?}", e);
        },
        Ok(()) => {
          println!("Successfully generated document.");
        }
      }
    },
    Commands::Inithandout { out_file } => {
      match init_handout(out_file) {
        Err(e) => {
          println!("Error creating handout: {:?}", e);
        },
        Ok(()) => {
          println!("Successfully created handout.");
        }
      }
    }
  }

  let args: Vec<String> = env::args().collect();

  if args.len() < 2 {
    println!("usage: cli [action]");
    println!("actions: handoutgen");
    return;
  }

  if args[1] == "handoutgen" {

      
  }
}