use std::path::PathBuf;

use crate::check::{CheckCategory, CheckReport, CheckWarning};
use crate::parser;
use crate::render;

pub fn run(file: PathBuf, _verbose: u8, quiet: bool) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(&file)?;
    let base_path = file.parent().unwrap_or(std::path::Path::new("."));
    let presentation = parser::parse(&content, base_path);

    if presentation.slides.is_empty() {
        anyhow::bail!("No slides found in {}", file.display());
    }

    let slide_count = presentation.slides.len();
    let file_name = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if !quiet {
        eprintln!(
            "Checking {} ({} slide{})...",
            file_name,
            slide_count,
            if slide_count == 1 { "" } else { "s" }
        );
    }

    let mut report = CheckReport::new();

    for (i, slide) in presentation.slides.iter().enumerate() {
        let slide_num = i + 1;
        for block in &slide.blocks {
            if let parser::Block::Diagram { content } = block {
                for warning_msg in render::diagram::check_diagram_routes(content) {
                    report.add(CheckWarning {
                        slide: slide_num,
                        category: CheckCategory::DiagramRouting,
                        message: warning_msg,
                    });
                }
            }
        }
    }

    if report.has_warnings() {
        if !quiet {
            report.print_detailed();
        }
        std::process::exit(1);
    } else {
        if !quiet {
            eprintln!("No issues found.");
        }
        Ok(())
    }
}
