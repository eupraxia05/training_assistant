use clap::Command;
use core_new::{
    CommandRunContext, CommandRunner, tui::Tui,
};
use cursive::backends::crossterm::Backend as CrosstermBackend;
use cursive::views::TextView;
use cursive::{Cursive, CursiveExt};

fn main() {
    let command_runner =
        CommandRunner::with_builtin_commands();

    let mut tui = Tui::default();

    let context = CommandRunContext {
        tui: Some(&mut tui),
    };

    let run_result = command_runner.run(
        "tacl",
        std::env::args(),
        context,
    );

    println!("{}", run_result);

    if tui.session_requested() {
        tui.run_session(CrosstermBackend::init);
    }
}
