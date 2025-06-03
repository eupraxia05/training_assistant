use std::env;

use handoutgen::generate_document;

fn main() {
  let args: Vec<String> = env::args().collect();

  if args.len() < 2 {
    println!("usage: cli [action]");
    println!("actions: handoutgen");
    return;
  }

  if args[1] == "handoutgen" {
    match generate_document() {
      Err(e) => {
        println!("Error generating document: {:?}", e);
      },
      Ok(()) => {
        println!("Successfully generated document.");
      }
    }  
  }
}