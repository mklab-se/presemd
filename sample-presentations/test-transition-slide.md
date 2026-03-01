---
title: "Transition Test: Slide"
@theme: dark
@transition: slide
---

# Slide Transition
Horizontal slide between slides

# Why Slide?

- Clear sense of direction
- Forward = slide left
- Backward = slide right

# Code Example

```rust
fn slide(progress: f32, direction: Direction) -> f32 {
    match direction {
        Forward => -progress,
        Backward => progress,
    }
}
```

## Summary

The slide transition pushes slides horizontally.
Navigate forward and backward to see the effect.
