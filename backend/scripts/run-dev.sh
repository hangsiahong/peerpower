#!/bin/bash

# Simple script to run the PeerPower backend in development mode

echo "🚀 Starting PeerPower Backend in development mode..."

# Set environment variables for development
export ENVIRONMENT=development
export HOST=0.0.0.0
export PORT=8080
export DATABASE_URL="mongodb://admin:password@localhost:27017/?authSource=admin"
export DATABASE_NAME=peerpower
export REDIS_URL=redis://:password@localhost:6379
export JWT_SECRET=dev-jwt-secret-change-in-production-please
export INSTANCE_ID=dev-backend-001
export REGION=local

# Start the backend
cargo run
