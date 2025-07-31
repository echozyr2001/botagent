# ByteBot Agent Rust Service

This is the Rust implementation of the ByteBot AI agent service, providing high-performance task management, AI model integration, and WebSocket communication.

## Docker Configuration

### Building the Image

```bash
# From the project root
docker build -t bytebot-agent-rs -f packages/bytebot-agent-rs/Dockerfile .
```

### Running the Container

```bash
docker run -d \
  --name bytebot-agent-rs \
  -p 9991:9991 \
  -e DATABASE_URL="postgresql://postgres:postgres@localhost:5432/bytebotdb" \
  -e ANTHROPIC_API_KEY="your-anthropic-key" \
  -e OPENAI_API_KEY="your-openai-key" \
  -e GOOGLE_API_KEY="your-google-key" \
  bytebot-agent-rs
```

### Environment Variables

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `DATABASE_URL` | PostgreSQL connection string | Yes | - |
| `ANTHROPIC_API_KEY` | Anthropic Claude API key | No | - |
| `OPENAI_API_KEY` | OpenAI GPT API key | No | - |
| `GOOGLE_API_KEY` | Google Gemini API key | No | - |
| `RUST_LOG` | Log level configuration | No | `info` |
| `RUST_BACKTRACE` | Enable backtraces on panic | No | `1` |

### Health Check

The service includes a built-in health check endpoint at `/health` that:
- Verifies database connectivity
- Returns service status and version information
- Provides database connection pool statistics

Health check endpoint: `http://localhost:9991/health`

### Multi-Stage Build

The Dockerfile uses a multi-stage build approach:

1. **Builder Stage**: Uses `rust:1.75-slim` to compile the application
2. **Runtime Stage**: Uses `debian:bookworm-slim` for a minimal production image

This approach significantly reduces the final image size while maintaining all necessary runtime dependencies.

### Security Features

- Runs as non-root user (`bytebot`)
- Minimal runtime dependencies
- No unnecessary packages in production image
- Proper file permissions and ownership

### Ports

- **9991**: HTTP API and WebSocket endpoints

### Volume Mounts

No persistent volumes are required for the agent service itself, but ensure your database is properly configured with persistent storage.