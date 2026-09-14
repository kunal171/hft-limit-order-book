# Current Architecture Flowcharts

These diagrams describe what the repository implements today. They are kept
separate from the target HFT architecture so planned components are not
mistaken for working runtime paths.

Editable high-level design diagram:

[`diagrams/current-system-hld.drawio`](diagrams/current-system-hld.drawio)

Maintenance rule: update this document and the editable Draw.io file whenever
a route, service, storage dependency, runtime boundary, or delivery status
changes.

Status legend:

```text
Implemented   Present and exercised by the project
In progress   Present on the current branch, with tests still being completed
Planned       Documented direction, not connected to the runtime yet
```

## Executable Topology

The project currently has two independent runtime paths: an Axum control-plane
API backed by PostgreSQL, and a synchronous in-memory simulator/replay CLI.

```mermaid
flowchart LR
    Client[HTTP client]
    Windmill[Windmill]

    subgraph ApiProcess[Axum API process]
        ApiBinary[src/bin/api.rs]
        Router[Axum router]
        Auth[Authentication middleware]
        ApiHandlers[Auth, account, and admin handlers]
        ApiBinary --> Router
        Router --> Auth
        Auth --> ApiHandlers
    end

    subgraph CliProcess[Simulator CLI process]
        Cli[src/main.rs]
        Scenario[Scenario and synthetic generators]
        Runner[Scenario runner]
        Book[In-memory OrderBook]
        Metrics[Book and trade metrics]
        Artifacts[JSON events, snapshot, and summary]
        Cli --> Scenario --> Runner --> Book
        Book --> Metrics
        Book --> Artifacts
        Metrics --> Artifacts
    end

    subgraph AnalysisProcess[Deterministic analysis]
        Scripts[Shell wrappers]
        Analyzer[Python run analyzer]
        Report[analysis.md]
        Scripts --> Analyzer --> Report
    end

    Postgres[(PostgreSQL)]

    Client --> ApiBinary
    ApiHandlers --> Postgres
    Windmill --> Scripts
    Scripts --> Cli
    Artifacts --> Analyzer
```

There is currently no connection from the Axum API to the `OrderBook`. Orders
submitted by network clients, a dedicated engine thread, bounded command
queues, and durable engine journaling are planned work.

## HTTP Routing And Middleware

```mermaid
flowchart TD
    Request[HTTP request] --> Root[Root Axum router]

    Root --> Health[GET /health]
    Root --> Signup[POST /auth/signup]
    Root --> Login[POST /auth/login]
    Root --> Logout[POST /auth/logout]

    Root --> SessionAuth[Bearer-session middleware]
    SessionAuth --> Me[GET /auth/me]
    SessionAuth --> AccountCreate[POST /accounts]
    SessionAuth --> AccountList[GET /accounts]

    SessionAuth --> AdminRole[Admin-role middleware]
    AdminRole --> AdminUsers[POST /admin/users]
    AdminRole --> AdminInstruments[POST /admin/instruments]

    Health --> Postgres[(PostgreSQL)]
    Signup --> Postgres
    Login --> Postgres
    Logout --> Postgres
    SessionAuth --> Postgres
    AccountCreate --> Postgres
    AccountList --> Postgres
    AdminUsers --> Postgres
    AdminInstruments --> Postgres
```

Middleware order matters for `/admin/*`: bearer authentication runs first and
inserts `AuthenticatedUser`; role authorization then reads that trusted value
and requires `role = admin`.

## Signup And Login

```mermaid
flowchart TD
    SignupRequest[Signup JSON] --> SignupValidate[Normalize and validate]
    SignupValidate --> HashPassword[Argon2 hash in spawn_blocking]
    HashPassword --> SignupTx[Begin PostgreSQL transaction]
    SignupTx --> InsertUser[Insert active trader]
    InsertUser --> InsertCredential[Insert password hash]
    InsertCredential --> SignupCommit[Commit]
    SignupCommit --> SignupResponse[201 trader response]

    LoginRequest[Login JSON] --> NormalizeEmail[Normalize email]
    NormalizeEmail --> LoadCredential[Load user and password hash]
    LoadCredential --> VerifyPassword[Argon2 verify in spawn_blocking]
    VerifyPassword --> ActiveCheck{User active?}
    ActiveCheck -->|No| LoginForbidden[403]
    ActiveCheck -->|Yes| GenerateToken[Generate random 256-bit token]
    GenerateToken --> HashToken[SHA-256 token hash]
    HashToken --> StoreSession[Store hash and expiry in sessions]
    StoreSession --> LoginResponse[Return raw token once]
```

Public signup never accepts a role. It always creates a trader. The initial
administrator is created separately by `src/bin/bootstrap_admin.rs`.

## Protected Request And Logout

```mermaid
flowchart TD
    ProtectedRequest[Protected HTTP request] --> ParseBearer[Parse Bearer token]
    ParseBearer --> TokenDigest[Calculate SHA-256 digest]
    TokenDigest --> SessionQuery[Join sessions and users in PostgreSQL]
    SessionQuery --> ValidSession{Exists, unrevoked, unexpired?}
    ValidSession -->|No| Unauthorized[401]
    ValidSession -->|Yes| ActiveUser{User active?}
    ActiveUser -->|No| Forbidden[403]
    ActiveUser -->|Yes| Extension[Insert AuthenticatedUser extension]
    Extension --> ProtectedHandler[Run protected handler]

    LogoutRequest[POST /auth/logout] --> LogoutBearer[Parse and hash token]
    LogoutBearer --> Revoke[Set revoked_at when not already revoked]
    Revoke --> NoContent[204, including repeated logout]
```

Only token hashes are stored. PostgreSQL remains the session authority; Redis
session caching is planned but not implemented.

## Account Creation And Listing

```mermaid
flowchart TD
    AccountRequest[Authenticated request] --> Identity[AuthenticatedUser extension]
    Identity --> Method{HTTP method}

    Method -->|POST| ParseAccount[Parse name only]
    ParseAccount --> NormalizeName[Trim and validate name]
    NormalizeName --> InsertAccount[Insert UUID v7, owner ID, active status]
    InsertAccount --> Duplicate{Unique owner and name conflict?}
    Duplicate -->|Yes| Conflict[409]
    Duplicate -->|No| Created[201 account]

    Method -->|GET| OwnerQuery[Select WHERE user_id = authenticated user]
    OwnerQuery --> AccountList[200 account list]
```

The client cannot provide `user_id`, `status`, or `id`. Ownership always comes
from authenticated middleware, and `UNIQUE (user_id, name)` permits different
users to reuse the same account name.

## Instrument Batch Creation

This flow is implemented and covered by PostgreSQL integration tests.

```mermaid
flowchart TD
    InstrumentRequest[Admin JSON array] --> Authenticated{Valid session?}
    Authenticated -->|No| Instrument401[401]
    Authenticated -->|Yes| IsAdmin{Role is admin?}
    IsAdmin -->|No| Instrument403[403]
    IsAdmin -->|Yes| BatchSize{1 to 100 items?}
    BatchSize -->|No| Instrument400[400]
    BatchSize -->|Yes| ValidateAll[Normalize and validate every instrument]
    ValidateAll -->|Invalid| Instrument400
    ValidateAll -->|Valid| InstrumentTx[Begin transaction]
    InstrumentTx --> InsertLoop[Insert each instrument as active]
    InsertLoop --> InsertResult{Insert result}
    InsertResult -->|More items| InsertLoop
    InsertResult -->|Unique conflict| Rollback[Drop transaction and roll back]
    Rollback --> Instrument409[409]
    InsertResult -->|Other database error| ErrorRollback[Roll back]
    ErrorRollback --> Instrument500[500]
    InsertResult -->|All inserted| InstrumentCommit[Commit transaction]
    InstrumentCommit --> Instrument201[201 instrument array]
```

The transaction makes the batch atomic: either every instrument is created or
none of the new rows remain.

## Matching Engine

The current engine is synchronous and owned directly by the caller.

```mermaid
flowchart TD
    Command[Add order] --> ValidateOrder{Quantity and ID valid?}
    ValidateOrder -->|No| OrderError[OrderBookError]
    ValidateOrder -->|Yes| AcceptedEvent[Record accepted event when enabled]
    AcceptedEvent --> Side{Buy or sell?}
    Side -->|Buy| BestAsk[Read lowest active ask]
    Side -->|Sell| BestBid[Read highest active bid]
    BestAsk --> Cross{Price crosses?}
    BestBid --> Cross
    Cross -->|No| Rest[Rest remaining quantity]
    Cross -->|Yes| Fifo[Take oldest order at best price]
    Fifo --> Fill[Execute minimum remaining quantity]
    Fill --> Trade[Create trade and update depth]
    Trade --> RestingFilled{Resting order filled?}
    RestingFilled -->|Yes| RemoveIndexes[Remove active order indexes]
    RestingFilled -->|No| KeepFront[Keep partial order at FIFO front]
    RemoveIndexes --> IncomingFilled{Incoming order filled?}
    KeepFront --> IncomingFilled
    IncomingFilled -->|No| Side
    IncomingFilled -->|Yes| TradeEvents[Record trade events when enabled]
    Rest --> TradeEvents
    TradeEvents --> Result[Return trades]
```

Current in-memory ownership:

```mermaid
flowchart LR
    OrderBook --> Bids[BTreeMap bids]
    OrderBook --> Asks[BTreeMap asks]
    Bids --> BidLevels[PriceLevel: FIFO order IDs and cached quantity]
    Asks --> AskLevels[PriceLevel: FIFO order IDs and cached quantity]
    OrderBook --> Orders[HashMap order ID to Order]
    OrderBook --> Locations[HashMap order ID to side and price]
    OrderBook --> Events[Vec BookEvent]
```

## Simulation, Artifacts, And Replay

```mermaid
flowchart TD
    CliArgs[CLI arguments] --> Mode{Simulation or replay?}

    Mode -->|Simulation| Commands[Predefined or synthetic commands]
    Commands --> Runner[Run commands sequentially]
    Runner --> Add[Add]
    Runner --> Cancel[Cancel]
    Runner --> Modify[Modify]
    Add --> Book[In-memory OrderBook]
    Cancel --> Book
    Modify --> Book
    Book --> Trades[Collected trades]
    Book --> Events[Collected events]
    Book --> Snapshot[Book snapshot]
    Trades --> Metrics[Trade metrics]
    Snapshot --> Metrics
    Events --> JsonArtifacts[Optional JSON artifacts]
    Snapshot --> JsonArtifacts
    Metrics --> JsonArtifacts

    Mode -->|Replay| EventFile[Load events JSON]
    EventFile --> Replay[Apply accepted, cancelled, and modified events]
    Replay --> RebuiltBook[Rebuilt OrderBook]
    RebuiltBook --> ReplayMetrics[Snapshot and metrics]
```

Trade events are not applied directly during replay. Replaying accepted and
modified orders deterministically produces those trades again.

## Current PostgreSQL Relationships

```mermaid
erDiagram
    USERS ||--o| USER_CREDENTIALS : has
    USERS ||--o{ SESSIONS : opens
    USERS ||--o{ ACCOUNTS : owns

    USERS {
        uuid id PK
        text display_name
        text email UK
        text status
        text role
        timestamptz created_at
    }

    USER_CREDENTIALS {
        uuid user_id PK,FK
        text password_hash
        timestamptz created_at
        timestamptz updated_at
    }

    SESSIONS {
        uuid id PK
        uuid user_id FK
        bytea token_hash UK
        timestamptz expires_at
        timestamptz revoked_at
        timestamptz created_at
    }

    ACCOUNTS {
        uuid id PK
        uuid user_id FK
        text name
        text status
        timestamptz created_at
    }

    INSTRUMENTS {
        uuid id PK
        text symbol UK
        asset_class asset_class
        text base_asset
        text quote_asset
        int price_scale
        int quantity_scale
        bigint tick_size
        bigint lot_size
        market_status status
        timestamptz created_at
    }
```

`instruments` is intentionally independent of users and accounts: an
instrument defines a market available to the whole trading system.

## Current Boundary Versus Planned HFT Runtime

```mermaid
flowchart LR
    subgraph Implemented[Implemented now]
        ControlPlane[Axum and PostgreSQL control plane]
        Simulator[CLI simulator]
        CurrentBook[Synchronous in-memory OrderBook]
        Replay[JSON event replay]
        ControlPlane
        Simulator --> CurrentBook --> Replay
    end

    subgraph Planned[Planned later]
        Gateway[Persistent order gateway]
        Queue[Bounded command queue]
        EngineThread[Dedicated single-writer engine thread]
        Journal[Checksummed durable journal]
        Kafka[Kafka event distribution]
        Projections[PostgreSQL projections]
        Gateway --> Queue --> EngineThread --> Journal --> Kafka --> Projections
    end
```

The next architectural bridge is not a database call from matching. It is a
bounded command path from a gateway to a dedicated engine owner, using local
validation snapshots derived from the control plane.
