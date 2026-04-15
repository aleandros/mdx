# Test: Par Fragment

```mermaid
sequenceDiagram
    participant Alice
    participant Bob
    participant Charlie
    par Notify both
        Alice->>Bob: Hello Bob
    and
        Alice->>Charlie: Hello Charlie
    end
```
