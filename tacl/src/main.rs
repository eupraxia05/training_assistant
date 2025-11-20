//! The command-line interface for Training Assistant.
use framework::prelude::*;
use tui::Tui;

fn main() -> Result<()> {
    let mut context = Context::new();
    context.add_plugin(DbPlugin);

    #[cfg(feature="billing")]
    context.add_plugin(billing::InvoicePlugin);

    #[cfg(feature="training")]
    context.add_plugin(training::TrainingPlugin);

    #[cfg(feature="db_commands")]
    context.add_plugin(db_commands::DbCommandsPlugin);

    context.startup()?;

    let mut command_args =
        std::env::args().collect::<Vec<_>>();

    // remove the initial executable name from args
    // todo: this isn't guaranteed to be the executable name, should probably check it's what we expect
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
            let tui_requested = if let Some(tui) = context.get_resource_mut::<Tui>() {
                true
            } else { false };

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

fn tui_session(context: &mut Context) -> std::result::Result<(), ()> {
    color_eyre::install().map_err(|_| ())?;

    let mut terminal = ratatui::init();
    let mut tui_state = TuiState::default();
    let result = {
        loop {
            terminal.draw(render).map_err(|_| ())?;
            let ev = crossterm::event::read().map_err(|_| ())?; 
            /*(update_fn)(context, &mut tui_state, &ev);
            if tui_state.should_quit() { 
                break Ok(());
            }*/
        }
    };
    ratatui::restore();

    result
}

fn render(frame: &mut ratatui::Frame) {
    frame.render_widget("hello world", frame.area());
}
