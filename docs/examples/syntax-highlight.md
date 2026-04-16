# Syntax Highlighting Demo

## Rust

```rust
use std::collections::HashMap;

fn main() {
    let mut scores: HashMap<&str, i32> = HashMap::new();
    scores.insert("Alice", 42);
    scores.insert("Bob", 99);

    for (name, score) in &scores {
        println!("{name}: {score}");
    }
}
```

## Python

```python
from dataclasses import dataclass
from typing import Optional

@dataclass
class User:
    name: str
    email: str
    age: Optional[int] = None

    def greet(self) -> str:
        return f"Hello, {self.name}!"

users = [User("Alice", "alice@example.com", 30)]
for user in users:
    print(user.greet())
```

## JavaScript / TypeScript

```javascript
async function fetchUsers(url) {
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const data = await response.json();
    return data.users.filter(u => u.active);
  } catch (err) {
    console.error("Failed to fetch:", err.message);
    return [];
  }
}
```

## Go

```go
package main

import (
    "fmt"
    "sync"
)

func worker(id int, wg *sync.WaitGroup, ch <-chan string) {
    defer wg.Done()
    for msg := range ch {
        fmt.Printf("Worker %d: %s\n", id, msg)
    }
}

func main() {
    ch := make(chan string, 10)
    var wg sync.WaitGroup
    for i := 0; i < 3; i++ {
        wg.Add(1)
        go worker(i, &wg, ch)
    }
    ch <- "hello"
    close(ch)
    wg.Wait()
}
```

## Shell

```bash
#!/bin/bash
set -euo pipefail

readonly LOG_DIR="/var/log/app"

cleanup() {
    echo "Cleaning up temp files..."
    rm -rf "${TMPDIR:-/tmp}/build-*"
}
trap cleanup EXIT

for file in "$LOG_DIR"/*.log; do
    if [[ -f "$file" ]]; then
        lines=$(wc -l < "$file")
        echo "$(basename "$file"): $lines lines"
    fi
done
```

## No Language Tag (fallback)

```
This block has no language specified.
It should render in plain dim monochrome.
No syntax highlighting here.
```

## Unknown Language

```brainfuck
++++++++[>++++[>++>+++>+++>+<<<<-]>+>+>->>+[<]<-]
```
