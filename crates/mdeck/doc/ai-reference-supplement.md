## CLI Command Reference

### Presenting

```bash
mdeck <file.md>                # Launch presentation (fullscreen)
mdeck <file.md> --windowed     # Launch in a window
mdeck <file.md> --slide 5      # Start on slide 5
mdeck <file.md> --overview     # Start in grid overview mode
mdeck <file.md> --check        # Validate presentation without launching GUI
```

### Format Specification

```bash
mdeck spec                     # Print full format specification
mdeck spec --short             # Print quick reference card
```

### Export

```bash
mdeck export <file.md>                          # Export slides as PNG (1920x1080)
mdeck export <file.md> --width 3840 --height 2160  # Export at custom resolution
mdeck export <file.md> --output-dir ./slides    # Export to specific directory
```

### Configuration

```bash
mdeck config show              # Display current configuration
mdeck config set <key> <value> # Set a config value
```

Available config keys:
- `defaults.theme` — default theme (`light`, `dark`, `nord`)
- `defaults.transition` — default transition (`fade`, `slide`, `none`)
- `defaults.aspect` — default aspect ratio (`16:9`, `4:3`, `16:10`)

### AI Commands

```bash
mdeck ai                       # Show AI status
mdeck ai status                # Show AI status (same as above)
mdeck ai enable                # Enable AI features
mdeck ai disable               # Disable AI features
mdeck ai config                # Interactive AI provider configuration
mdeck ai test                  # Test AI integration
```

### AI Image Generation

```bash
mdeck ai generate <file.md>              # Generate all AI images in a presentation
mdeck ai generate <file.md> --force      # Skip confirmation prompt
mdeck ai generate <file.md> --style name # Override the image style
mdeck ai generate-image --prompt "..."   # Generate a single image
mdeck ai generate-image --prompt "..." --style "watercolor"
mdeck ai generate-image --prompt "..." --icon   # Generate as icon
mdeck ai generate-image --prompt "..." --output path.png
```

### AI Style Management

```bash
mdeck ai style list                        # List all defined styles
mdeck ai style add <name> <description>    # Add a named image style
mdeck ai style add <name> <desc> --icon    # Add a named icon style
mdeck ai style add -i                      # Interactive style creation (AI-assisted)
mdeck ai style remove <name>               # Remove a named style
mdeck ai style remove <name> --icon        # Remove a named icon style
mdeck ai style clear                       # Remove all styles and reset defaults
mdeck ai style set-default <name>          # Set default image style
mdeck ai style set-icon-default <name>     # Set default icon style
mdeck ai style show-defaults               # Show current default styles
```

## AI Image Generation Guide

### Marking Images for Generation

Use `image-generation` as the image path to mark an image for AI generation:

```markdown
![A futuristic cityscape at sunset](image-generation)
```

The alt text becomes the image prompt. Leave alt text empty for auto-prompting from slide context (requires chat capability):

```markdown
![](image-generation)
```

### Image Style Control

Image styles control the visual aesthetic of all generated images. Styles can be set at multiple levels (highest priority first):

1. **Per-file frontmatter:** `@image-style: watercolor` (name or literal description)
2. **Config default:** `mdeck ai style set-default <name>`
3. **Hardcoded fallback:** A built-in default style

For icons (used in architecture diagrams):
1. **Per-file frontmatter:** `@icon-style: flat-design`
2. **Config default:** `mdeck ai style set-icon-default <name>`
3. **Hardcoded fallback:** A built-in default icon style

### Diagram Icon Generation

In architecture diagrams, use `icon: generate-image` with a `prompt` to mark a node for AI icon generation:

````markdown
```@architecture
- Gateway (icon: generate-image, prompt: "An API gateway router icon", pos: 1,2)
- Database (icon: database, pos: 2,2)
- Gateway -> Database: queries
```
````

### The `mdeck ai generate` Workflow

1. Write your presentation with `image-generation` markers and/or diagram icon prompts
2. Run `mdeck ai generate <file.md>`
3. The command detects orientation automatically (horizontal for full-slide images, vertical for side-panel layouts)
4. It applies the configured image style
5. The markdown file is rewritten in-place, replacing `image-generation` with actual file paths

### Tips for AI Agents Writing Presentations

- Use descriptive alt text for image generation prompts — be specific about the scene, mood, and composition
- Set `@image-style` in frontmatter when the presentation has a consistent visual theme
- Use `+` list markers for incremental reveal (appears on forward press)
- Use `*` list markers to group items with the previous `+` reveal step
- Use `-` for static items that are always visible
- Keep slide content concise — presentations are meant to be visual aids, not documents
- Use the `---` separator or 3+ blank lines between slides
- Architecture diagrams with `+`/`*` markers create animated build-up sequences
- Use `@layout: two-column` with `+++` separator for side-by-side comparisons
