---
title: "Layout Test: Code Slides"
@theme: dark
@transition: fade
---

# Layout Test: Code Slides
Focused tests for the code layout


# Hello World in Rust

```rust
fn main() {
    println!("Hello, world!");
}
```


# Python with Line Highlights

```python {2,4-5}
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)
# This line is also highlighted

print(fibonacci(10))
```


# Longer Code Block

```javascript
class PresentationEngine {
    constructor(slides) {
        this.slides = slides;
        this.currentIndex = 0;
        this.transition = 'fade';
    }

    next() {
        if (this.currentIndex < this.slides.length - 1) {
            this.currentIndex++;
            this.render();
        }
    }

    previous() {
        if (this.currentIndex > 0) {
            this.currentIndex--;
            this.render();
        }
    }

    render() {
        const slide = this.slides[this.currentIndex];
        console.log(`Rendering slide: ${slide.title}`);
    }
}
```
