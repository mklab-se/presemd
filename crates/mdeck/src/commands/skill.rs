//! AI agent skill information for mdeck.
//!
//! `mdeck ai skill`             — print setup guide
//! `mdeck ai skill --emit`      — print skill markdown file to stdout
//! `mdeck ai skill --reference` — print full reference documentation

const SPEC: &str = include_str!("../../doc/mdeck-spec.md");
const SUPPLEMENT: &str = include_str!("../../doc/ai-reference-supplement.md");

pub fn run(emit: bool, reference: bool) {
    if emit {
        print_skill_file();
    } else if reference {
        print_reference();
    } else {
        print_setup_guide();
    }
}

fn print_setup_guide() {
    println!(
        r#"mdeck AI Skill Setup
====================

mdeck is a markdown-based presentation tool. A skill helps AI agents
create stunning presentations from standard markdown files.

To create the skill file, run:

  mdeck ai skill --emit > ~/.claude/skills/mdeck.md

Or ask your AI agent:

  "Use `mdeck ai skill --emit` to set up a skill for creating presentations"

The skill instructs the AI agent to run `mdeck ai skill --reference` at
runtime to fetch the full format specification and documentation, so the
agent always has up-to-date syntax details without bloating the skill file
itself."#
    );
}

fn print_skill_file() {
    print!(
        r#"---
name: mdeck
description: Markdown-based presentation tool — create slide decks with 15+ visualization types, AI-generated images, themes, transitions, and automatic layout inference.
---

# mdeck — Markdown Presentations

Use mdeck when the user needs to create, edit, or present slide decks
written in markdown.

## Getting current documentation

IMPORTANT: Before writing ANY presentation content, you MUST run this
command to load the complete format specification and reference:

```bash
mdeck ai skill --reference
```

Do NOT skip this step. The spec contains essential details about slide
separation, layout inference, directives, diagram syntax, visualization
types, and incremental reveal that you need to write correct presentations.

## Quick command reference

- `mdeck <file.md>` — launch presentation
- `mdeck <file.md> --check` — validate without launching
- `mdeck ai generate <file.md>` — generate AI images
- `mdeck export <file.md>` — export slides as PNG images
- `mdeck spec` — print format specification
- `mdeck spec --short` — print quick reference card
- `mdeck ai status` — show AI configuration status
- `mdeck ai config` — configure AI providers

## Workflow

1. Run `mdeck ai skill --reference` to load the full spec
2. Write or edit the presentation markdown
3. Run `mdeck <file.md> --check` to validate
4. If using AI images, run `mdeck ai generate <file.md>`
"#
    );
}

fn print_reference() {
    println!("# mdeck Reference Documentation\n");
    println!("## Format Specification\n");
    println!("{SPEC}");
    println!("\n{SUPPLEMENT}");
}
