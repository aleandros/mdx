# Styled Sequence Diagram

```mermaid
sequenceDiagram
    participant Client
    participant Server
    participant DB
    Client->>Server: Request
    Server->>DB: Query
    DB->>Server: Result
    Server->>Client: Response
    style Client fill:#ff9900,stroke:#333
    style Server stroke:#336633
    style DB fill:#6699ff
    linkStyle 0 stroke:#ff3333
    linkStyle 3 stroke:#33cc33
```
