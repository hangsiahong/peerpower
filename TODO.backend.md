# TODO: PeerPower Backend Development

## 🎯 Phase 1: Core Infrastructure Setup

### 1.1 Project Initialization

- [x] Create Rust project with Cargo.toml
- [x] Choose web framework (Axum)
- [x] **Setup Clean Architecture structure**
  - [x] Domain layer (entities, repositories, services)
  - [x] Infrastructure layer (database, external APIs)
  - [x] Presentation layer (HTTP handlers)
  - [x] Shared utilities and error types
- [x] **Stateless application design (no local state)**
- [x] **12-factor app compliance**
- [x] Configure environment variables (.env)
- [x] Setup logging (tracing/log crate with JSON output)
- [x] Docker setup for development
- [x] **Health check endpoints (/health, /ready)**
- [x] **Graceful shutdown handling**

### 1.2 Database Setup

- [x] MongoDB connection setup **with connection pooling**
- [x] **Database connection retry logic with backoff**
- [x] **Read replica support for scaling reads**
- [x] Database schemas/models:
  - [x] Provider model (user_id, sim_type, status, location, etc.)
  - [x] Message model (content, recipient, provider_id, status, etc.)
  - [x] Job model (message_id, provider_id, created_at, completed_at)
  - [x] User model (phone, did, evm_address, reputation_score)
  - [ ] Audit log model
- [x] **Database indexes for performance at scale**
- [x] **Distributed locks using MongoDB or Redis**

---

## 🎯 Phase 2: Authentication & User Management

### 2.1 Phone Number Authentication

- [x] Phone number validation (Cambodia format)
- [x] OTP generation and verification (for onboarding)
- [x] JWT token generation
- [x] Session management
- [x] Rate limiting for auth endpoints

### 2.2 DID Integration

- [x] DID generation/validation
- [x] Link DID to phone number
- [x] Optional EVM wallet binding
- [x] User profile management API

---

## 🎯 Phase 3: Provider Registry Service

### 3.1 Provider Management

- [ ] Provider registration endpoint
- [ ] SIM type detection/registration (Smart, Metfone, Cellcard)
- [ ] Provider status tracking (online/offline/busy)
- [ ] Location-based provider registration (optional)
- [ ] Provider heartbeat/health check system

### 3.2 Provider Matching Algorithm

- [ ] Carrier-to-carrier matching logic
- [ ] Provider availability scoring
- [ ] Load balancing across providers
- [ ] Fallback provider selection
- [ ] Provider rate limiting (max messages per day/hour)

---

## 🎯 Phase 4: Message Dispatcher Service

### 4.1 Message Queue System

- [ ] Job queue implementation (Redis or in-memory)
- [ ] Message priority levels
- [ ] Retry mechanism with exponential backoff
- [ ] Dead letter queue for failed messages
- [ ] Job timeout handling

### 4.2 API Endpoints

- [ ] `POST /api/messages/send` - Submit SMS job
- [ ] Message validation (content, recipient)
- [ ] Anti-spam content filtering
- [ ] Khmer word dictionary integration
- [ ] Rate limiting per client
- [ ] API key management for clients

### 4.3 Firebase Cloud Messaging Integration

- [ ] FCM setup and configuration
- [ ] Send job notifications to provider devices
- [ ] Handle FCM delivery confirmations
- [ ] Retry failed FCM deliveries

---

## 🎯 Phase 5: Proof-of-Delivery Service

### 5.1 Delivery Tracking

- [ ] Receive delivery reports from mobile apps
- [ ] Message status updates (sent, delivered, failed)
- [ ] Delivery confirmation validation
- [ ] Hidden message ID/token system for traceability
- [ ] Webhook system for client notifications

### 5.2 Anti-Fraud Measures

- [ ] Detect fake delivery reports
- [ ] Provider reputation scoring
- [ ] Suspicious activity detection
- [ ] Blacklist management
- [ ] Delivery success rate tracking

---

## 🎯 Phase 6: Baray Payment Integration

### 6.1 Payment Processing

- [ ] Baray API integration setup
- [ ] Payment webhook handling
- [ ] Payment verification system
- [ ] Receipt generation and storage
- [ ] Failed payment handling

### 6.2 PPT Token Management

- [ ] Selendra Network integration
- [ ] ERC20 contract interaction
- [ ] Token minting on payment confirmation
- [ ] Wallet balance tracking
- [ ] Transaction history

---

## 🎯 Phase 7: Incentive Engine

### 7.1 Reward Calculation

- [ ] PPT reward calculation per message
- [ ] Dynamic pricing based on demand/supply
- [ ] Bonus systems (leaderboard, loyalty)
- [ ] Provider scoring algorithm
- [ ] Reward distribution scheduler

### 7.2 Payout Systems

- [ ] Airtime top-up integration (Smart, Metfone APIs)
- [ ] Cash withdrawal via Baray
- [ ] Minimum payout thresholds
- [ ] Payout history and reporting

---

## 🎯 Phase 8: Monitoring & Admin

### 8.1 Admin Dashboard APIs

- [ ] System statistics endpoints
- [ ] Provider management APIs
- [ ] Message analytics and reporting
- [ ] Revenue and payout tracking
- [ ] System health monitoring

### 8.2 Observability

- [ ] Prometheus metrics integration
- [ ] Error tracking and alerting
- [ ] Performance monitoring
- [ ] Database query optimization
- [ ] API response time tracking

---

## 🎯 Phase 9: Security & Compliance

### 9.1 Security Measures

- [ ] Input validation and sanitization
- [ ] SQL injection prevention
- [ ] Rate limiting and DDoS protection
- [ ] API authentication and authorization
- [ ] Data encryption at rest and in transit

### 9.2 Audit & Compliance

- [ ] Comprehensive audit logging
- [ ] GDPR compliance considerations
- [ ] Data retention policies
- [ ] Backup and disaster recovery
- [ ] Privacy protection measures

---

## 🎯 Phase 10: API Documentation & Testing

### 10.1 Testing

- [ ] Unit tests for core business logic
- [ ] Integration tests for APIs
- [ ] Load testing for high traffic scenarios
- [ ] End-to-end testing
- [ ] Mock services for external dependencies

### 10.2 Documentation

- [ ] API documentation (OpenAPI/Swagger)
- [ ] Developer guides
- [ ] Deployment documentation
- [ ] Troubleshooting guides

---

## 🚀 Deployment & DevOps

### 11.1 Production Setup

- [ ] Production environment configuration
- [ ] CI/CD pipeline setup
- [ ] Database backup strategy
- [ ] SSL certificate setup
- [ ] Domain and DNS configuration

### 11.2 Scaling Preparation

- [ ] Horizontal scaling architecture
- [ ] Load balancer configuration
- [ ] Database sharding strategy (future)
- [ ] Caching layer implementation
- [ ] CDN setup for static assets

---

## 📋 Development Priorities

**Start Here (Critical Path):**

1. Project setup + Database models
2. Phone auth + Provider registry
3. Basic message dispatcher
4. FCM integration
5. Simple proof-of-delivery

**Then:** 6. Baray payment integration 7. PPT token system 8. Admin APIs 9. Security hardening 10. Production deployment

---

## 🔧 Technical Decisions Needed

- [ ] Choose between Actix-web vs Axum
- [ ] Redis vs RabbitMQ for message queue
- [ ] Authentication strategy (JWT vs sessions)
- [ ] Deployment platform (AWS, DigitalOcean, etc.)
- [ ] Monitoring stack (Prometheus + Grafana vs alternatives)

---

## 📝 Notes

- Keep Khmer language support in mind for all user-facing content
- Design APIs to be mobile-app friendly (efficient, minimal data)
- Plan for Cambodia's internet connectivity challenges
- Consider offline-first design for mobile apps
- Prepare for potential carrier blocking countermeasures
