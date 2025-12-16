//! The command-line interface for Training Assistant.
use framework::prelude::*;
use tui::Tui;

fn main() -> Result<()> {
    let mut context = Context::new();
    context.add_plugin(DbPlugin)?;

    #[cfg(feature="tui")]
    context.add_plugin(tui::TuiPlugin)?;

    #[cfg(feature="billing")]
    context.add_plugin(billing::InvoicePlugin)?;

    #[cfg(feature="training")]
    context.add_plugin(training::TrainingPlugin)?;

    #[cfg(feature="db_commands")]
    context.add_plugin(db_commands::DbCommandsPlugin)?;

    context.startup()?;

    let mut command_args =
        std::env::args().collect::<Vec<_>>();

    // remove the initial executable name from args
    // TODO: this isn't guaranteed to be the executable name, should probably check it's what we expect
    command_args.remove(0);

    let response = context
        .execute(
            shlex::try_join(
                command_args
                    .iter()
                    .map(|e| e.as_str()),
            )
            .expect("failed to join args")
            .as_str(),
        );
    
    match response {
        Ok(r) => {
            if let Some(text) = r.text() {
                println!("{}", text);
            }
            let tui_requested = context.has_resource::<Tui>();

            if tui_requested {
                tui::run_tui(&mut context).expect("failed to run tui session"); 
            }
        },
        Err(e) => {
            println!("error: {:?}", e);
        }
    }

    Ok(())
}

