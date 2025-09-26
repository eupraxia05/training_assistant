use std::{env, path::PathBuf};
use clap::{Parser, Subcommand};
use training::DatabaseConnection;

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
  },
  Clients {
    #[command(subcommand)]
    command: ClientCommands
  },
  Invoice {
    #[clap(short = 'o', long, env)]
    out_file: PathBuf
  }
}

#[derive(Subcommand)]
enum ClientCommands {
    List,
    Add {
        #[clap(short = 'n', long, env)]
        name: String
    },
    Remove {
        #[clap(short = 'i', long, env)]
        id: i64
    },
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
    },
    Commands::Clients { command } => {
        let mut db_connection = DatabaseConnection::open_default().expect("couldn't open database");
        match command {
            ClientCommands::List => {
                let clients = db_connection.clients().expect("couldn't get clients");
                if clients.len() == 0 {
                    println!("No clients in database.");
                } else {
                    for client in clients {
                        println!("{:?}: {}", client.id(), client.name());
                    }
                }
            },
            ClientCommands::Add { name } => {
                db_connection.add_client(name).expect("couldn't add client");
            },
            ClientCommands::Remove { id } => {
                db_connection.remove_client(training::ClientId(id)).expect("couldn't remove client");
            },
        }
    },
    Commands::Invoice { out_file } => {
        invoice::create_invoice(out_file); 
    }
  }
}
