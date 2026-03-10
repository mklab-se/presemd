mod app;
mod banner;
mod check;
mod cli;
mod commands;
mod config;
mod incident_log;
mod parser;
mod prompt;
mod render;
mod theme;

use clap::{CommandFactory, Parser};
use colored::Colorize;

fn main() {
    clap_complete::CompleteEnv::with_factory(cli::Cli::command).complete();

    let cli = cli::Cli::parse();

    if cli.no_color {
        colored::control::set_override(false);
    }

    if let Err(e) = cli.run() {
        eprintln!("{} {e}", "Error:".red().bold());
        std::process::exit(1);
    }
}
