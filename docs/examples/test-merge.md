# Test: Branch and Merge

```mermaid
graph TD
    A[Start] --> B{Check}
    B -->|Yes| C[Process]
    B -->|No| D[Skip]
    C --> E[Done]
    D --> E
```
