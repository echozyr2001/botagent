# ByteBot Agent Rust

This is the Rust implementation of the ByteBot AI agent service, designed to replace the TypeScript version while maintaining full API compatibility.

## Features

- **Database Connection Management**: PostgreSQL connection pooling with retry logic and health checks
- **Migration System**: Automatic database schema migration compatible with existing Prisma schema
- **Configuration Management**: Environment-based configuration with sensible defaults
- **Error Handling**: Comprehensive error types with HTTP response conversion
- **Health Monitoring**: Database health checks and connection pool statistics

## Setup

### Prerequisites

- Rust 1.75 or later
- PostgreSQL database
- Environment variables configured (see `.env.example`)

### Configuration

Copy the example environment file and configure your settings:

```bash
cp .env.example .env
```

Key configuration options:

- `DATABASE_URL`: PostgreSQL connection string
- `LOG_LEVEL`: Logging level (debug, info, warn, error)
- `HOST` and `PORT`: Server binding configuration

### Database Setup

The service will automatically:

1. Create the database if it doesn't exist
2. Run migrations to set up the schema
3. Perform health checks to ensure connectivity

The database schema is designed to be compatible with the existing Prisma schema used by the TypeScript version.

### Running

```bash
# Development
cargo run

# Production build
cargo build --release
./target/release/bytebot-agent-rs
```

### Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_database_manager_creation
```

## Architecture

### Database Layer

- **DatabaseManager**: Handles connection pooling, health checks, and connection lifecycle
- **MigrationRunner**: Manages database schema migrations and version tracking
- **Error Handling**: Comprehensive error types for database operations

### Configuration

- Environment-based configuration with validation
- Support for optional services (Redis, AI APIs)
- Flexible server configuration

### Key Components

```
src/
├── config.rs          # Configuration management
├── database/          # Database layer
│   ├── connection.rs  # Connection pooling and health checks
│   └── migrations.rs  # Schema migration system
├── error.rs           # Error types and HTTP conversion
└── main.rs           # Application entry point
```

## Database Schema

The service uses the same database schema as the TypeScript version:

- **Task**: Main task entity with status, priority, and execution tracking
- **Message**: AI conversation messages with content blocks
- **User**: User management (Better Auth integration)
- **Session**: User session management
- **Summary**: Hierarchical task summaries
- **File**: File attachments for tasks

## Health Checks

The service provides health check endpoints and monitoring:

- Database connectivity verification
- Connection pool statistics
- Migration status tracking

## Performance Features

- Connection pooling with configurable limits
- Retry logic for database connections
- Efficient migration system
- Structured logging for monitoring

## Development

### Adding New Migrations

1. Create a new SQL file in `migrations/` with timestamp prefix
2. Follow the existing schema naming conventions
3. Test migrations with `cargo test`

### Error Handling

All database operations use the `DatabaseError` type which converts to appropriate HTTP responses. Add new error variants as needed for specific error conditions.

### Testing

Tests are organized by module and include:

- Unit tests for individual components
- Integration tests for database operations
- Mock-based testing for external dependencies

## Compatibility

This service is designed to be a drop-in replacement for the TypeScript version:

- Same REST API endpoints
- Identical database schema
- Compatible WebSocket events
- Same configuration options