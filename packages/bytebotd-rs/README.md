# ByteBot Desktop Automation Daemon Rust Service

This is the Rust implementation of the ByteBot desktop automation daemon, providing high-performance computer control capabilities including mouse, keyboard, screen capture, and file operations.

## Docker Configuration

### Building the Image

```bash
# From the project root
docker build -t bytebotd-rs -f packages/bytebotd-rs/Dockerfile .
```

### Running the Container

```bash
docker run -d \
  --name bytebotd-rs \
  -p 9990:9990 \
  --shm-size="2g" \
  --privileged \
  -e DISPLAY=:0 \
  bytebotd-rs
```

### Environment Variables

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `DISPLAY` | X11 display configuration | Yes | `:0` |
| `RUST_LOG` | Log level configuration | No | `info` |
| `RUST_BACKTRACE` | Enable backtraces on panic | No | `1` |

### Health Check

The service includes a built-in health check endpoint at `/health` that:
- Verifies service availability
- Returns capability information
- Provides service status and version

Health check endpoint: `http://localhost:9990/health`

### Multi-Stage Build

The Dockerfile uses a multi-stage build approach:

1. **Builder Stage**: Uses `rust:1.75-slim` with X11 development libraries to compile the application
2. **Runtime Stage**: Uses `ubuntu:22.04` with full desktop environment for automation capabilities

### Desktop Environment

The container includes:
- **Xvfb**: Virtual framebuffer for headless operation
- **X11VNC**: VNC server for remote desktop access
- **XFCE4**: Lightweight desktop environment
- **Supervisor**: Process management for multiple services

### Services Managed by Supervisor

1. **Xvfb**: Virtual display server
2. **X11VNC**: VNC server for remote access
3. **XFCE4-Session**: Desktop environment
4. **bytebotd-rs**: Main automation service

### Security Features

- Desktop session runs as non-root user (`user`)
- Proper file permissions and ownership
- Isolated desktop environment

### Ports

- **9990**: HTTP API for computer automation and MCP endpoints

### Capabilities

The service provides the following automation capabilities:
- **Screenshot**: Capture screen images
- **Mouse Control**: Move, click, drag, scroll operations
- **Keyboard Control**: Text input and key combinations
- **File Operations**: Read and write files with proper validation

### Volume Mounts

- `/tmp/bytebot-screenshots`: Screenshot storage (optional)
- `/home/user`: User home directory for desktop session (optional)

### Container Requirements

- **Privileged Mode**: Required for desktop automation
- **Shared Memory**: 2GB recommended for desktop environment
- **Display**: X11 display configuration for desktop session

### MCP Integration

The service includes Model Context Protocol (MCP) support for tool integration with AI models, providing a standardized interface for computer automation capabilities.