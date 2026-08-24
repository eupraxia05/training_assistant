use clap::ArgMatches;
use clap::Args;
use clap::Command;
use clap::builder::IntoResettable;
use std::collections::HashMap;
use std::ffi::OsString;

pub mod config;
pub mod db;
pub mod tui;

pub struct CommandRunner {
    commands: HashMap<String, RegisteredCommand>,
}

struct RegisteredCommand {
    command: Command,
    run_command_fn: RunCommandFn,
}

#[derive(Default)]
pub struct CommandRunContext<'a> {
    pub tui: Option<&'a mut tui::Tui>,
}

pub type RunCommandFn =
    fn(&ArgMatches, CommandRunContext) -> String;

impl CommandRunner {
    pub fn with_builtin_commands() -> Self {
        let mut result = Self::default();
        tui::add_builtin_commands(&mut result);
        config::add_builtin_commands(&mut result);
        result
    }

    pub fn add_command(
        &mut self,
        command: Command,
        run_command_fn: RunCommandFn,
    ) {
        self.commands.insert(
            command.get_name().into(),
            RegisteredCommand {
                command,
                run_command_fn,
            },
        );
    }

    pub fn run<I, T>(
        &self,
        bin_name: &'static str,
        args: I,
        context: CommandRunContext,
    ) -> String
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let mut command = Command::new(bin_name)
            .subcommand_required(true);

        for (_, subcommand) in &self.commands {
            command = command.subcommand(
                subcommand.command.clone(),
            );
        }

        let match_result =
            command.try_get_matches_from(args);

        match &match_result {
            Err(e) => {
                return e.to_string();
            }
            Ok(matches) => {
                if let Some(subcommand_name) =
                    matches.subcommand_name()
                {
                    for (name, command) in
                        &self.commands
                    {
                        if name == subcommand_name
                            && let Some(
                                subcommand_matches,
                            ) = matches
                                .subcommand_matches(
                                    subcommand_name,
                                )
                        {
                            let response = (command
                                .run_command_fn)(
                                &subcommand_matches,
                                context,
                            );
                            return response;
                        }
                    }
                    return "No matching subcommand."
                        .into();
                } else {
                    return "Subcommand required."
                        .into();
                }
            }
        }
    }
}

impl Default for CommandRunner {
    fn default() -> Self {
        Self {
            commands: HashMap::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::{ArgMatches, Command};

    use crate::{CommandRunContext, CommandRunner};

    fn speak_command(
        _arg_matches: &ArgMatches,
        _context: CommandRunContext,
    ) -> String {
        "woof".into()
    }

    #[test]
    fn command_run_test() {
        let mut runner = CommandRunner::default();
        runner.add_command(
            Command::new("speak"),
            speak_command,
        );

        let context = CommandRunContext::default();

        assert_eq!(
            runner.run(
                "tacl",
                &["tacl", "speak"],
                context
            ),
            "woof"
        );
    }
}
