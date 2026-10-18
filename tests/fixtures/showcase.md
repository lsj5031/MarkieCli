# From notes to a shareable document

**One Markdown source.** Native rendering for everyday engineering work.

## A small release plan

- [x] Preserve the content
- [x] Review diagrams and mathematics
- [ ] Share the finished document

| Deliverable | Format | Why it helps |
| --- | --- | --- |
| Design notes | SVG | Keep diagrams sharp |
| Team update | PNG | Drop into a conversation |
| Review packet | PDF | Share a fixed layout |

## Keep the implementation readable

```rust
fn main() {
    let formats = ["svg", "png", "pdf"];
    for format in formats {
        println!("Ready to share: {format}");
    }
}
```

> Good tools preserve meaning, make constraints clear, and produce something
> useful to share.
