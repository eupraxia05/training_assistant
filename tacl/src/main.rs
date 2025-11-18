//! The command-line interface for Training Assistant.

use billing::InvoicePlugin;
use framework::prelude::*;

fn main() -> Result<()> {
    let mut context = Context::new();
    context
        .add_plugin(DbPlugin)
        .add_plugin(InvoicePlugin);

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
            if r.tui_requested() {
                tui_session(r.tui_render_fn().unwrap(), r.tui_update_fn().unwrap()).expect("failed to run tui session");
            }
        },
        Err(e) => {
            println!("error: {:?}", e);
        }
    }

    Ok(())
}

fn tui_session(render_fn: TuiRenderFn, update_fn: TuiUpdateFn) -> std::result::Result<(), ()> {
    color_eyre::install().map_err(|_| ())?;

    let mut terminal = ratatui::init();
    let mut tui_state = TuiState::default();
    let result = {
        loop {
            terminal.draw(render_fn).map_err(|_| ())?;
            let ev = crossterm::event::read().map_err(|_| ())?; 
            (update_fn)(&mut tui_state, &ev);
            if tui_state.should_quit() { 
                break Ok(());
            }
        }
    };
    ratatui::restore();

    result
}

fn render(frame: &mut ratatui::Frame) {
    frame.render_widget("hello world", frame.area());
}
