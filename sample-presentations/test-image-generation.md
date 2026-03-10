---
title: "Image Generation Test"
author: "mdeck"
@theme: dark
@transition: fade
@image-style: "Vibrant, cinematic lighting with rich colors and dramatic composition. No text."
@icon-style: "Clean flat icon, solid colors, no gradients, no text."
---

# AI Image Generation

Testing `mdeck ai generate` workflow


## Full-Slide Image

![A futuristic cityscape at sunset with flying vehicles and neon lights](image-generation)


## Bullet Slide with Side Image

### Key Features

- Automatic image generation
- Style management via config
- Per-presentation style overrides
+ Smart orientation detection
+ Diagram icon generation

![A creative workspace with monitors showing code and design tools](image-generation)


## Code Slide with Side Image

```python
from mdeck import generate

# Generate all images in a presentation
generate("slides.md", style="cinematic")
```

![A robot artist painting on a canvas](image-generation)


## Auto-Prompt (Empty Alt Text)

> "The best way to predict the future is to invent it."
>
> — Alan Kay

![](image-generation)


## Architecture Diagram with Generated Icons

```@diagram
- Gateway: API Gateway (icon: generate-image, prompt: "An API gateway router icon", pos: 1, 2)
- Auth: Authentication (icon: generate-image, prompt: "A security lock shield icon", pos: 2, 1)
- App: Application (icon: generate-image, prompt: "A running application gear icon", pos: 2, 3)
- DB: Database (icon: database, pos: 3, 2)

Gateway -> Auth: validate
Gateway -> App: forward
Auth -> App: token
App -> DB: query
```
