# -----------------------------------------------------------------------------
# ByteBot Desktop Automation Daemon Rust Service Dockerfile
# Multi-stage build for optimized production image with desktop automation
# -----------------------------------------------------------------------------

# -----------------------------------------------------------------------------
# Stage 1: Build stage
# -----------------------------------------------------------------------------
FROM rust:1.75-slim as builder

# Install system dependencies needed for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    # X11 development libraries for desktop automation
    libx11-dev \
    libxtst-dev \
    libxinerama-dev \
    libxi-dev \
    libxt-dev \
    libxrandr-dev \
    libxkbcommon-dev \
    libxkbcommon-x11-dev \
    # Build tools
    cmake \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace Cargo.toml and Cargo.lock first for better caching
COPY Cargo.toml Cargo.lock ./

# Copy shared library
COPY packages/bytebot-shared-rs/ ./packages/bytebot-shared-rs/

# Copy desktop daemon source
COPY packages/bytebotd-rs/ ./packages/bytebotd-rs/

# Build the application in release mode
WORKDIR /app/packages/bytebotd-rs
RUN cargo build --release

# -----------------------------------------------------------------------------
# Stage 2: Runtime stage with desktop environment
# -----------------------------------------------------------------------------
FROM ubuntu:22.04

# Set non-interactive installation
ARG DEBIAN_FRONTEND=noninteractive
# Configure display for X11 applications
ENV DISPLAY=:0

# -----------------------------------------------------------------------------
# System dependencies installation
# -----------------------------------------------------------------------------
RUN apt-get update && apt-get install -y \
    # X11 / VNC for desktop automation
    xvfb x11vnc xauth x11-xserver-utils \
    x11-apps sudo software-properties-common \
    # Desktop environment (minimal)
    xfce4-session xfce4-panel xfce4-desktop \
    dbus wmctrl \
    # Display manager with autologin capability
    lightdm \
    # Development tools
    curl wget git vim \
    # Utilities
    supervisor netcat-openbsd \
    # Runtime libraries for desktop automation
    libx11-6 \
    libxtst6 \
    libxinerama1 \
    libxi6 \
    libxt6 \
    libxrandr2 \
    libxkbcommon0 \
    libxkbcommon-x11-0 \
    # SSL and crypto libraries
    ca-certificates \
    libssl3 \
    # Image processing libraries
    libpng16-16 \
    libjpeg8 \
    # Clipboard utilities
    xclip \
    # Remove unneeded packages
    && apt-get remove -y light-locker xfce4-screensaver xfce4-power-manager || true \
    # Clean up to reduce image size
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Create machine-id for dbus
RUN mkdir -p /run/dbus && \
    dbus-uuidgen --ensure=/etc/machine-id

# -----------------------------------------------------------------------------
# User setup and permissions
# -----------------------------------------------------------------------------
# Create non-root user for desktop session
RUN useradd -ms /bin/bash user && echo "user ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# Set up dbus permissions
RUN mkdir -p /var/run/dbus && \
    chmod 755 /var/run/dbus && \
    chown user:user /var/run/dbus

# Create screenshot directory
RUN mkdir -p /tmp/bytebot-screenshots && \
    chown -R user:user /tmp/bytebot-screenshots

# Set up user home directory
RUN mkdir -p /home/user/.config /home/user/.local/share /home/user/.cache \
    && chown -R user:user /home/user/.config /home/user/.local /home/user/.cache

# -----------------------------------------------------------------------------
# Copy application binary
# -----------------------------------------------------------------------------
# Copy the built binary from builder stage
COPY --from=builder /app/packages/bytebotd-rs/target/release/bytebotd-rs /usr/local/bin/bytebotd-rs

# Make binary executable
RUN chmod +x /usr/local/bin/bytebotd-rs

# -----------------------------------------------------------------------------
# Supervisor configuration for managing services
# -----------------------------------------------------------------------------
RUN mkdir -p /etc/supervisor/conf.d

# Create supervisor configuration
COPY <<EOF /etc/supervisor/conf.d/supervisord.conf
[supervisord]
nodaemon=true
user=root
logfile=/var/log/supervisor/supervisord.log
pidfile=/var/run/supervisord.pid

[program:xvfb]
command=/usr/bin/Xvfb :0 -screen 0 1280x720x24 -ac +extension GLX +render -noreset
user=root
autostart=true
autorestart=true
stdout_logfile=/var/log/supervisor/xvfb.log
stderr_logfile=/var/log/supervisor/xvfb.log

[program:x11vnc]
command=/usr/bin/x11vnc -display :0 -nopw -listen localhost -xkb -ncache 10 -ncache_cr -forever -shared
user=root
autostart=true
autorestart=true
stdout_logfile=/var/log/supervisor/x11vnc.log
stderr_logfile=/var/log/supervisor/x11vnc.log

[program:xfce4-session]
command=/usr/bin/xfce4-session
user=user
environment=HOME="/home/user",USER="user",DISPLAY=":0"
autostart=true
autorestart=true
stdout_logfile=/var/log/supervisor/xfce4.log
stderr_logfile=/var/log/supervisor/xfce4.log

[program:bytebotd-rs]
command=/usr/local/bin/bytebotd-rs
user=user
environment=HOME="/home/user",USER="user",DISPLAY=":0",RUST_LOG="info",RUST_BACKTRACE="1"
autostart=true
autorestart=true
stdout_logfile=/var/log/supervisor/bytebotd-rs.log
stderr_logfile=/var/log/supervisor/bytebotd-rs.log
EOF

# Create log directory
RUN mkdir -p /var/log/supervisor && \
    chown -R user:user /var/log/supervisor

# -----------------------------------------------------------------------------
# Port configuration and runtime
# -----------------------------------------------------------------------------
# Expose the service port
EXPOSE 9990

# Health check configuration
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:9990/health || exit 1

# Set working directory
WORKDIR /home/user

# Start supervisor to manage all services
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/conf.d/supervisord.conf", "-n"]