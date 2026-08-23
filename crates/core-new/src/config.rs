use std::{
    arch::x86_64::_SIDD_MASKED_POSITIVE_POLARITY,
    ffi::OsString,
    fs::{self, File},
    io::{PipeWriter, Write},
    path::PathBuf,
    thread::ScopedJoinHandle,
};

use clap::{Arg, ArgMatches, Command};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{CommandRunContext, CommandRunner};

#[derive(Serialize, Deserialize)]
pub struct Config {
    db_path: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            db_path: build_default_db_path(),
        }
    }
}

pub fn add_builtin_commands(
    runner: &mut CommandRunner,
) {
    runner.add_command(
        Command::new("config")
            .subcommand(
                Command::new("set")
                    .arg(Arg::new("key"))
                    .arg(Arg::new("value")),
            )
            .subcommand(
                Command::new("revert")
                    .arg(Arg::new("key")),
            ),
        run_config_command,
    );
}

fn run_config_command(
    arg_matches: &ArgMatches,
    context: CommandRunContext,
) -> String {
    if let Some(subcommand) = arg_matches.subcommand()
    {
        match subcommand.0 {
            "set" => {
                let mut config = load_config();

                let mut config_value =
                    serde_json::to_value(config)
                        .unwrap();

                let key = subcommand
                    .1
                    .get_one::<String>("key")
                    .unwrap();

                let mut value =
                    config_value.get_mut(key).unwrap();

                // need to set value from string

                match subcommand
                    .1
                    .get_one::<String>("key")
                    .unwrap()
                    .as_str()
                {
                    "db_path" => {
                        config.db_path = subcommand
                            .1
                            .get_one::<String>("value")
                            .unwrap()
                            .into();
                        save_config(&config);
                        "Done.".into()
                    }
                    k => {
                        format!(
                            "Unrecognized key: {}",
                            k
                        )
                    }
                }
            }
            "revert" => {
                let mut config = load_config();
                let default_config = Config::default();
                match subcommand
                    .1
                    .get_one::<String>("key")
                    .unwrap()
                    .as_str()
                {
                    "db_path" => {
                        config.db_path =
                            default_config.db_path;
                        save_config(&config);
                        "Done.".into()
                    }
                    k => {
                        format!(
                            "Unrecognized key: {}",
                            k
                        )
                    }
                }
            }
            _ => {
                // shouldn't receive any other subcommand from clap
                panic!()
            }
        }
    } else {
        let config_path = build_config_file_path();

        let mut result = format!(
            "Config stored at: {}\n",
            config_path.display()
        );

        if !fs::exists(config_path).unwrap() {
            result += "File does not exist, using default config.\n";
        }

        result += "\n";

        let config = load_config();
        result += format!(
            "db_path: {}",
            config.db_path.display()
        )
        .as_str();

        result
    }
}

fn build_config_dir_path() -> PathBuf {
    let dirs = ProjectDirs::from(
        "",
        "",
        "training_assistant",
    )
    .unwrap();

    dirs.config_dir().into()
}

fn build_config_file_path() -> PathBuf {
    build_config_dir_path().join("config.json")
}

fn build_default_db_path() -> PathBuf {
    let dirs = ProjectDirs::from(
        "",
        "",
        "training_assistant",
    )
    .unwrap();

    dirs.data_dir().join("data.db")
}

pub fn load_config() -> Config {
    let config_path = build_config_file_path();
    if !fs::exists(config_path).unwrap() {
        return Config::default();
    }
    serde_json::from_reader::<_, Config>(
        File::open(build_config_file_path()).unwrap(),
    )
    .unwrap()
}

pub fn save_config(config: &Config) {
    fs::create_dir_all(build_config_dir_path())
        .unwrap();

    serde_json::to_writer_pretty(
        File::create(build_config_file_path())
            .unwrap(),
        config,
    )
    .unwrap();
}
