# Test: Nested Fragments

```mermaid
sequenceDiagram
    participant Client
    participant Server
    participant DB
    loop Retry 3 times
        Client->>Server: Request
        alt Success
            Server->>DB: Query
            DB-->>Server: Result
            Server-->>Client: 200 OK
        else Failure
            Server-->>Client: 500 Error
        end
    end
```
