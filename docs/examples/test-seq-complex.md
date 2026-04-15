# Test: Complex Sequence (API Auth Flow)

```mermaid
sequenceDiagram
    autonumber
    participant Client
    participant Gateway
    participant Auth
    participant API
    participant DB

    Client->>Gateway: POST /login
    Gateway->>Auth: Validate credentials
    activate Auth
    Auth->>DB: SELECT user
    DB-->>Auth: User record

    alt Valid credentials
        Auth->>Auth: Generate JWT
        Auth-->>Gateway: 200 + token
        deactivate Auth
        Gateway-->>Client: Set-Cookie: token

        Client->>Gateway: GET /data
        Gateway->>Auth: Verify token
        activate Auth
        Auth-->>Gateway: Token valid
        deactivate Auth
        Gateway->>API: Forward request
        activate API
        API->>DB: Query data
        DB-->>API: Results
        API-->>Gateway: 200 OK
        deactivate API
        Gateway-->>Client: 200 OK

    else Invalid credentials
        Auth-->>Gateway: 401 Unauthorized
        deactivate Auth
        Gateway-->>Client: 401 Unauthorized
    end

    Note over Client,Gateway: Connection closed
```
