# Syntax Highlighting Test

This file tests syntax highlighting for different languages in Markie.

## Rust

```rust
fn main() {
    let greeting = "Hello, world!";
    println!("{}", greeting);
    
    // A comment
    if true {
        do_something();
    }
}
```

## Python

```python
def fibonacci(n):
    if n <= 1:
        return n
    else:
        return fibonacci(n-1) + fibonacci(n-2)

print(fibonacci(10))
```

## JavaScript

```javascript
const greet = (name) => {
    console.log(`Hello, ${name}!`);
};

greet("Markie");
```

## No Language

```
Just some plain text
inside a code block.
```

## Inline Code

Here is some `inline code` and `more code with symbols: < > &`.
