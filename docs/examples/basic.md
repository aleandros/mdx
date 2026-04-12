# Basic Markdown Rendering

This document demonstrates **mdx**'s core markdown rendering capabilities.

## Text Formatting

Here is some **bold text**, some *italic text*, and some `inline code`.
You can also combine **bold and *nested italic*** for emphasis.

Visit [the Rust website](https://www.rust-lang.org) for more about the language.

## Code Blocks

```rust
fn main() {
    let greeting = "Hello from mdx!";
    println!("{}", greeting);
}
```

```python
def fibonacci(n):
    a, b = 0, 1
    for _ in range(n):
        a, b = b, a + b
    return a
```

## Lists

Unordered:

- First item
- Second item
- Third item

Ordered:

1. Step one
2. Step two
3. Step three

---

That's the basics. Try `mdx docs/examples/basic.md` to see it rendered.
