use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
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

    /// Validate presentation and report problems without launching GUI
    #[arg(long, global = false)]
    pub check: bool,

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
    /// Manage AI features (shows status when run without a subcommand)
    Ai {
        #[command(subcommand)]
        command: Option<AiCommands>,
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
    /// Test AI integration by sending a message
    Test {
        /// Message to send (default: "Say hello in one sentence.")
        message: Option<String>,
    },
    /// Enable AI features for mdeck
    Enable,
    /// Disable AI features for mdeck
    Disable,
    /// Open AI configuration file in your editor
    Config,
    /// Manage image styles
    Style {
        #[command(subcommand)]
        command: StyleCommands,
    },
    /// Generate a single image from a prompt
    GenerateImage(GenerateImageArgs),
    /// Generate AI images for a presentation
    Generate {
        /// Markdown file to process
        file: PathBuf,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
        /// Override the image style
        #[arg(long)]
        style: Option<String>,
    },
}

#[derive(Args)]
pub struct GenerateImageArgs {
    /// Image prompt
    #[arg(long)]
    pub prompt: String,
    /// Named style or literal description to apply
    #[arg(long)]
    pub style: Option<String>,
    /// Output file path
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Generate as icon (square, transparent bg)
    #[arg(long)]
    pub icon: bool,
}

#[derive(Subcommand)]
pub enum StyleCommands {
    /// Add or update a named image style
    Add {
        /// Style name
        name: String,
        /// Style description
        description: String,
        /// Add as icon style instead of image style
        #[arg(long)]
        icon: bool,
    },
    /// Remove a named style
    Remove {
        /// Style name
        name: String,
        /// Remove from icon styles instead of image styles
        #[arg(long)]
        icon: bool,
    },
    /// List all defined styles
    List,
    /// Remove all styles and reset defaults
    Clear,
    /// Set the default image style (used when no style is specified)
    SetDefault {
        /// Name of an existing style
        name: String,
    },
    /// Set the default icon style (used for diagram icon generation)
    SetIconDefault {
        /// Name of an existing icon style
        name: String,
    },
    /// Show the current default styles (including hardcoded fallbacks)
    ShowDefaults,
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
            Some(Commands::Ai { command }) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(crate::commands::ai::run(command, self.quiet))
            }
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
                    if self.check {
                        return crate::commands::check::run(file, self.verbose, self.quiet);
                    }
                    crate::app::run(file, self.windowed, self.slide, self.overview, self.quiet)
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
