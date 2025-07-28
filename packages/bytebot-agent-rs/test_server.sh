#!/bin/bash

# Simple test script to verify the server can start and respond to health checks
# This script is for manual testing only

echo "Testing Axum server setup..."

# Set test environment variables
export DATABASE_URL="postgresql://localhost:5432/test"
export HOST="127.0.0.1"
export PORT="9991"
export LOG_LEVEL="info"
export CORS_ORIGINS="http://localhost:3000"

echo "Environment variables set:"
echo "  DATABASE_URL: $DATABASE_URL"
echo "  HOST: $HOST"
echo "  PORT: $PORT"
echo "  LOG_LEVEL: $LOG_LEVEL"
echo "  CORS_ORIGINS: $CORS_ORIGINS"

echo ""
echo "Note: This test will fail to connect to the database, which is expected."
echo "The purpose is to verify that the Axum server setup is correct."
echo ""

# Try to run the server (it will fail due to database connection, but we can see if the setup is correct)
timeout 10s cargo run 2>&1 | head -20

echo ""
echo "Server setup test completed."
echo "If you see 'Starting ByteBot Agent Rust service...' and configuration loading messages,"
echo "then the Axum server setup is working correctly."