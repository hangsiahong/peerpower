# PeerPower Backend

A decentralized SMS relay network backend built with Rust and Axum.

## 🚀 Quick Start

### Prerequisites

- Rust 1.75 or later
- MongoDB (local or remote)
- Redis (local or remote)

### Development Setup

1. **Clone and navigate to backend:**

   ```bash
   cd backend/
   ```

2. **Copy environment template:**

   ```bash
   cp .env.example .env
   ```

3. **Edit `.env` with your configuration:**

   ```bash
   vim .env  # or your preferred editor
   ```

4. **Run with Docker Compose (recommended):**

   ```bash
   docker-compose up -d  # Start MongoDB + Redis
   ./scripts/run-dev.sh  # Start backend
   ```

5. **Or run manually:**
   ```bash
   # Make sure MongoDB and Redis are running
   cargo run
   ```

### Test the API

```bash
# Health check
curl http://localhost:8080/health

# Readiness check
curl http://localhost:8080/ready

# API info
curl http://localhost:8080/
```

## 📁 Project Structure

```
src/
├── main.rs              # Application entry point
├── config/              # Configuration management
├── domain/              # Business logic layer
│   ├── entities/        # Core business entities
│   ├── repositories/    # Repository traits
│   └── services/        # Domain services
├── infrastructure/      # External integrations
│   ├── database/        # MongoDB implementations
│   ├── messaging/       # FCM, Redis queue
│   ├── payments/        # Baray integration
│   └── blockchain/      # Selendra integration
├── presentation/        # HTTP layer
│   ├── handlers/        # API route handlers
│   └── middleware/      # Custom middleware
└── shared/              # Common utilities
    ├── errors.rs        # Error types
    └── mod.rs           # Shared types & utils
```

## 🏗️ Architecture

This backend follows **Clean Architecture** principles:

- **Domain Layer**: Business logic, entities, and rules
- **Infrastructure Layer**: External services (database, APIs)
- **Presentation Layer**: HTTP handlers and middleware
- **Shared Layer**: Common utilities and error types

## 🔧 Configuration

All configuration is loaded from environment variables. See `.env.example` for all available options.

### Key Environment Variables

| Variable         | Description               | Default  |
| ---------------- | ------------------------- | -------- |
| `DATABASE_URL`   | MongoDB connection string | Required |
| `REDIS_URL`      | Redis connection string   | Required |
| `JWT_SECRET`     | Secret for JWT tokens     | Required |
| `FCM_SERVER_KEY` | Firebase server key       | Optional |
| `BARAY_API_KEY`  | Baray payment API key     | Optional |

## 🐳 Docker

### Build the image:

```bash
docker build -t peerpower-backend .
```

### Run with Docker Compose:

```bash
docker-compose up
```

This starts:

- PeerPower Backend (port 8080)
- MongoDB (port 27017)
- Redis (port 6379)
- Mongo Express UI (port 8081)
- Redis Commander UI (port 8082)

## 🔍 Monitoring

### Health Endpoints

- `GET /health` - Service health check
- `GET /ready` - Readiness check with dependencies

### Logging

- Structured JSON logging
- Request tracing with correlation IDs
- Configurable log levels via `RUST_LOG`

## 🧪 Testing

```bash
# Run unit tests
cargo test

# Run with coverage
cargo test --all-features

# Run specific test
cargo test test_name
```

## 📝 API Documentation

Once running, the API provides:

- Health checks at `/health` and `/ready`
- Root endpoint at `/` with service info
- Future API endpoints will be at `/api/v1/*`

## 🚢 Deployment

### Production Checklist

- [ ] Set strong `JWT_SECRET`
- [ ] Configure production MongoDB
- [ ] Configure production Redis
- [ ] Set up proper CORS policies
- [ ] Configure SSL/TLS
- [ ] Set up monitoring and alerting
- [ ] Configure backup strategies

### Environment-specific configs:

- **Development**: Local services, debug logging
- **Staging**: Shared services, info logging
- **Production**: Managed services, error logging

## 🤝 Contributing

1. Create feature branch
2. Make changes following Rust conventions
3. Add tests for new functionality
4. Ensure `cargo check` and `cargo test` pass
5. Submit pull request

## 📋 TODO

See `../TODO.backend.md` for detailed development roadmap.

Next steps:

1. Implement repository traits and database layer
2. Add authentication and user management
3. Implement message dispatcher service
4. Add FCM integration
5. Implement provider registry

## 🆘 Troubleshooting

### Common Issues

**Build errors:**

```bash
cargo clean && cargo build
```

**Database connection:**

- Ensure MongoDB is running
- Check `DATABASE_URL` format
- Verify network connectivity

**Redis connection:**

- Ensure Redis is running
- Check `REDIS_URL` format
- Verify Redis is accessible

### Logs

Check application logs for detailed error information:

```bash
RUST_LOG=debug cargo run
```
