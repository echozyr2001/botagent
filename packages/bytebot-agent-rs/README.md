# ByteBot Agent Rust Service

This is the Rust implementation of the ByteBot AI agent service, designed to replace the existing TypeScript/Node.js version while maintaining full API compatibility.

## Features Implemented

### Task 5.1: Axum Web Server with Middleware ✅

The following components have been implemented:

#### 1. Main Application Setup
- **Axum Router**: Modern async web framework with proper routing
- **Graceful Shutdown**: Handles SIGTERM and CTRL+C signals properly
- **Configuration Management**: Environment-based configuration with `.env` support
- **Structured Logging**: Tracing-based logging with configurable levels

#### 2. Middleware Stack
- **CORS Middleware**: Configured to match existing TypeScript service (`origin: '*'`)
- **Request Tracing**: HTTP request/response logging with structured output
- **Error Handling**: Centralized error handling with proper HTTP status codes

#### 3. Health Check Endpoints
- **`/health`**: Basic health check endpoint
- **`/api/health`**: Health check with API prefix (matches existing pattern)
- **Database Health**: Includes database connectivity status in health response
- **Service Information**: Returns service version, timestamp, and pool statistics

#### 4. Application State Management
- **Shared State**: `AppState` struct containing configuration and database manager
- **Arc-wrapped**: Thread-safe shared state across handlers
- **Database Integration**: Integrated with existing database layer

## Configuration

The service uses environment variables for configuration:

```bash
DATABASE_URL=postgresql://postgres:postgres@postgres:5432/bytebotdb
HOST=0.0.0.0
PORT=9991
LOG_LEVEL=info
CORS_ORIGINS=http://localhost:3000,http://localhost:9992
```

## API Compatibility

The server maintains compatibility with the existing TypeScript service:

- **Port**: Default 9991 (same as TypeScript version)
- **CORS**: Allows all origins with same methods (`GET`, `POST`, `PUT`, `DELETE`, `OPTIONS`, `PATCH`)
- **Health Endpoints**: Same response format and structure
- **Error Handling**: JSON error responses with proper HTTP status codes

## Health Check Response Format

```json
{
  "status": "healthy",
  "timestamp": "2025-01-27T12:00:00Z",
  "version": "0.1.0",
  "service": "bytebot-agent-rs",
  "database": {
    "connected": true,
    "pool_stats": {
      "size": 10,
      "idle": 8
    }
  }
}
```

## Testing

Run unit tests (excluding database integration tests):
```bash
cargo test --lib -- --skip database::tests --skip server::tests
```

Build the service:
```bash
cargo build
```

## Next Steps

This implementation provides the foundation for:
- Task management REST API endpoints (Task 5.2)
- Message management API endpoints (Task 5.3)
- Model listing and configuration endpoints (Task 5.4)
- WebSocket gateway for real-time updates (Task 6.1-6.2)

The middleware stack and application structure are now ready to support these additional features.