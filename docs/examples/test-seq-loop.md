# Test: Loop Fragment

```mermaid
sequenceDiagram
    participant Alice
    participant Bob
    loop Every minute
        Alice->>Bob: Ping
        Bob-->>Alice: Pong
    end
```
