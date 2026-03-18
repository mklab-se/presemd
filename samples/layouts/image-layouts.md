---
title: "Layout Test: Image Split Layouts"
@theme: dark
@transition: fade
---

# Layout Test: Image Split Layouts
Testing image side-panel rendering in Bullet, Code, Quote, and Content layouts


# Bullet + Image

- First point about this topic
- Second point with more detail
- Third point wrapping up
- Fourth point for good measure

![Poker scene](../images/poker-1.png)


# Code + Image

```rust
fn main() {
    println!("Hello, world!");
    let x = 42;
    println!("The answer is {x}");
}
```

![Poker scene](../images/poker-2.png)


# Quote + Image

> The best way to predict the future is to invent it.

-- Alan Kay

![Saloon scene](../images/saloon-horizontal.png)


# Content + Image

## Mixed Content Slide

This slide has a heading, a paragraph, and an image.

It should render as a content+image split layout.

![Poker table](../images/poker-3.png)


# Bullet + Image (Ordered)

1. Step one of the process
2. Step two continues
3. Step three follows naturally
4. Final step completes

![Poker scene](../images/poker-4.png)


# Code + Image (Python)

```python
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

for i in range(10):
    print(fibonacci(i))
```

![Saloon](../images/saloon-vertical.png)


# Image Only (No Change)

![Poker scene](../images/poker-1.png)

This should still use the Image layout.


# Gallery (No Change)

![Scene one](../images/poker-1.png)
![Scene two](../images/poker-2.png)
