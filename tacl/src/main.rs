use std::{env, path::PathBuf};
use clap::{Parser, Subcommand};
use training::{DatabaseConnection, TrainerId, ClientId};

#[derive(Subcommand)] 
enum Commands {
    Db {
        #[command(subcommand)]
        command: DbCommands
    },
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
  Trainer {
      #[command(subcommand)]
      command: TrainerCommands
  },
  Invoice {
    #[command(subcommand)]
    command: InvoiceCommands
  },
  New {
    #[clap(long, env)]
    table: String
  },
  Set {
    #[clap(long, env)]
    table: String,

    #[clap(long, env)]
    id: i64,

    #[clap(long, env)]
    field: String,

    #[clap(long, env)]
    value: String
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

#[derive(Subcommand)]
enum TrainerCommands {
    List,
    Add {
        #[clap(short = 'n', long, env)]
        name: String
    },
    Remove {
        #[clap(short = 'n', long, env)]
        id: i64
    },
    Edit {
        #[clap(long, env)]
        id: i64
    },
    Set {
        #[clap(long, env)]
        id: i64,

        #[clap(long, env)]
        field: String,

        #[clap(long, env)]
        value: String,
    }
}

#[derive(Subcommand)]
enum DbCommands {
    Erase,
}

#[derive(Subcommand)]
enum InvoiceCommands {
    New {
        #[clap(long, env)]
        invoice_number: String
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
    Commands::Invoice { command } => {
        //let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");
        //invoice::create_invoice(&mut db_connection, out_file, TrainerId(trainer_id), ClientId(client_id));
        process_invoice_command(command);

    },
    Commands::Trainer { command } => {
        process_trainer_command(command);
    },
    Commands::Db { command } => {
        match command {
            DbCommands::Erase => {
                let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");
                db_connection.erase();
            }
        }
    },
    Commands::New { table } => {
        process_new_command(table);
    },
    Commands::Set { table, id, field, value } => {
        process_set_command(table, id, field, value);
    }
  }
}

fn process_trainer_command(command: TrainerCommands) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");

    match command {
        TrainerCommands::List => {
            let trainers = db_connection.trainers().expect("couldn't get clients");
            if trainers.len() == 0 {
                println!("No trainers in database.");
            } else {
                for trainer in trainers {
                    println!("{:?}: {}", trainer.id(), trainer.name());
                }
            }
        },
        TrainerCommands::Add { name } => {
           db_connection.add_trainer(name).expect("Couldn't add trainer"); 
        },
        TrainerCommands::Remove { id } => {
            db_connection.remove_trainer(TrainerId(id)).expect("Couldn't remove trainer!");
        },
        TrainerCommands::Edit { id } => {
            let trainer_metadata = db_connection.get_trainer_metadata(TrainerId(id)).expect("Couldn't get trainer metadata");
            println!("{:?}", trainer_metadata);
        },
        TrainerCommands::Set { id, field, value } => {
            db_connection.set_trainer_metadata_field(TrainerId(id), field, value).expect("Couldn't set field");
        }
    }
}

fn process_invoice_command(command: InvoiceCommands) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");
    
    match command {
        InvoiceCommands::New { invoice_number } => {
            db_connection.add_invoice(invoice_number).expect("Couldn't add invoice");
        }
    }
}

fn process_new_command(table: String) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");

    db_connection.insert_new_into_table(table).expect("couldn't insert new row!");
}

fn process_set_command(table: String, id: i64, field: String, value: String) {
    let mut db_connection = DatabaseConnection::open_default().expect("Couldn't open database connection");

    db_connection.set_field_in_table(table, id, field, value).expect("couldn't set field!");
}
