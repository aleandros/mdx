# Test: Activation

```mermaid
sequenceDiagram
    participant Client
    participant Server
    Client->>Server: Request
    activate Server
    Server-->>Client: Response
    deactivate Server
```
