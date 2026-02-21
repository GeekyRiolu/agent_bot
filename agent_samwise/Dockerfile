# Build stage
FROM rust:latest as builder


WORKDIR /workspace

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the API binary in release mode
RUN cargo build --bin api --release

# Runtime stage - use minimal image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the compiled binary from builder
COPY --from=builder /workspace/target/release/api ./api

# Create non-root user for security
RUN useradd -m -u 1001 appuser && chown -R appuser:appuser /app
USER appuser

# Expose port (Cloud Run expects 8080 by default, but we use env var)
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:${PORT:-8080}/health || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV PORT=8080

# Run the API server
CMD ["./api"]
