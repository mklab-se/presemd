use crate::cli::ConfigCommands;
use crate::config::Config;
use anyhow::Result;
use colored::Colorize;

pub fn run(cmd: ConfigCommands) -> Result<()> {
    match cmd {
        ConfigCommands::Show => show(),
        ConfigCommands::Set { key, value } => set(&key, &value),
    }
}

fn show() -> Result<()> {
    let config = Config::load_or_default();
    let path = Config::path()?;

    println!(
        "{} {}\n",
        "Config:".bold(),
        path.display().to_string().dimmed()
    );

    match &config.defaults {
        Some(defaults) => {
            println!("{}", "defaults:".bold());
            println!(
                "  {} {}",
                "theme:".bold(),
                defaults.theme.as_deref().unwrap_or("(not set)")
            );
            println!(
                "  {} {}",
                "transition:".bold(),
                defaults.transition.as_deref().unwrap_or("(not set)")
            );
            println!(
                "  {} {}",
                "aspect:".bold(),
                defaults.aspect.as_deref().unwrap_or("(not set)")
            );
            println!(
                "  {} {}",
                "start_mode:".bold(),
                defaults.start_mode.as_deref().unwrap_or("(not set)")
            );
        }
        None => {
            println!("{} (not set)", "defaults:".bold());
        }
    }

    println!();

    match ailloy::config::Config::load().ok().and_then(|c| {
        c.default_chat_node().ok().map(|(id, node)| {
            (
                id.to_string(),
                format!("{:?}", node.provider),
                node.model.clone(),
            )
        })
    }) {
        Some((id, provider, model)) => {
            println!("{}", "ai (via ailloy):".bold());
            println!("  {} {}", "node:".bold(), id.cyan());
            println!("  {} {}", "provider:".bold(), provider);
            if let Some(model) = model {
                println!("  {} {}", "model:".bold(), model);
            }
        }
        None => {
            println!(
                "{} (not set — run {})",
                "ai:".bold(),
                "ailloy config".cyan()
            );
        }
    }

    Ok(())
}

fn set(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load_or_default();
    config.set(key, value)?;
    let path = config.save()?;

    println!(
        "{} Set {} = {}",
        "Done!".green().bold(),
        key.bold(),
        value.cyan()
    );
    println!("  Saved to {}", path.display().to_string().dimmed());

    Ok(())
}
