---
title: "Transition Test: Fade"
@theme: dark
@transition: fade
---

# Fade Transition
Cross-dissolve between slides

# Why Fade?

- Smooth and unobtrusive
- Works with any content type
- Classic presentation style

# Code Example

```rust
fn fade(progress: f32) -> f32 {
    ease_in_out(progress)
}
```

## Summary

The fade transition cross-dissolves from one slide to the next.
Navigate forward and backward to see the effect.
