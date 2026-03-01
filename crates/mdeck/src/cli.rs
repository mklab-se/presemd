use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mdeck")]
#[command(author, version, about)]
#[command(long_about = "A markdown-based presentation tool.\n\n\
    Write your slides in standard markdown and present them beautifully.\n\n\
    Examples:\n  \
    mdeck slides.md              Launch presentation (fullscreen)\n  \
    mdeck slides.md --windowed   Launch in a window\n  \
    mdeck spec                   Print format specification\n  \
    mdeck spec --short           Print quick reference card")]
#[command(propagate_version = true)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Markdown file to present
    pub file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Launch in a window instead of fullscreen
    #[arg(long, global = false)]
    pub windowed: bool,

    /// Start on a specific slide (1-indexed)
    #[arg(long, global = false)]
    pub slide: Option<usize>,

    /// Start in grid overview mode
    #[arg(long, global = false)]
    pub overview: bool,

    /// Increase output verbosity (-v for debug, -vv for trace)
    #[arg(short, long, action = ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress non-essential output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configure AI provider for enhanced features
    Ai {
        #[command(subcommand)]
        command: AiCommands,
    },

    /// View and modify configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Generate shell completions
    Completion {
        /// Target shell
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Export slides as PNG images
    Export {
        /// Markdown file to export
        file: PathBuf,

        /// Output directory for PNG files
        #[arg(short, long, default_value = "export")]
        output_dir: PathBuf,

        /// Export width in pixels
        #[arg(long, default_value = "1920")]
        width: u32,

        /// Export height in pixels
        #[arg(long, default_value = "1080")]
        height: u32,
    },

    /// Generate diagram icons using AI image generation
    GenerateIcons {
        /// Markdown file containing diagrams
        file: PathBuf,
    },

    /// Print the mdeck markdown format specification
    Spec {
        /// Print a concise quick-reference card instead of the full spec
        #[arg(long)]
        short: bool,
    },

    /// Show version information
    Version,
}

#[derive(Subcommand)]
pub enum AiCommands {
    /// Set up AI provider for enhanced features
    Init,

    /// Show current AI provider configuration
    Status,

    /// Remove AI configuration
    Remove,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Display current configuration
    Show,

    /// Set a configuration value
    Set {
        /// Configuration key (e.g. defaults.theme, defaults.transition, defaults.aspect)
        key: String,

        /// Value to set
        value: String,
    },
}

#[derive(Clone, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

impl Cli {
    pub fn run(self) -> anyhow::Result<()> {
        match self.command {
            Some(Commands::Ai { command }) => crate::commands::ai::run(command),
            Some(Commands::Config { command }) => crate::commands::config::run(command),
            Some(Commands::Completion { shell }) => {
                crate::commands::completion::run(shell);
                Ok(())
            }
            Some(Commands::Export {
                file,
                output_dir,
                width,
                height,
            }) => crate::commands::export::run(file, output_dir, width, height),
            Some(Commands::GenerateIcons { file }) => {
                if !file.exists() {
                    anyhow::bail!("File not found: {}", file.display());
                }
                crate::commands::generate_icons::run(&file)
            }
            Some(Commands::Spec { short }) => {
                crate::commands::spec::run(short);
                Ok(())
            }
            Some(Commands::Version) => {
                crate::banner::print_banner_with_version();
                Ok(())
            }
            None => {
                if let Some(file) = self.file {
                    if !file.exists() {
                        anyhow::bail!("File not found: {}", file.display());
                    }
                    crate::app::run(file, self.windowed, self.slide, self.overview)
                } else {
                    use clap::CommandFactory;
                    let mut cmd = Self::command();
                    cmd.print_help()?;
                    println!();
                    Ok(())
                }
            }
        }
    }
}
