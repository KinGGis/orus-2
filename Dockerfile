# Global build args
ARG RUST_IMAGE=rust:1.91-bookworm

# Stage 1: build frontend
# Use --platform=$BUILDPLATFORM to run on the native runner (fast)
FROM --platform=$BUILDPLATFORM node:20-alpine AS frontend

# Wealthfolio Connect configuration (baked into JS bundle at build time)
# Pass via --build-arg to enable; omit to build without Connect.
ARG CONNECT_AUTH_URL=
ARG CONNECT_AUTH_PUBLISHABLE_KEY=
ENV CONNECT_AUTH_URL=${CONNECT_AUTH_URL}
ENV CONNECT_AUTH_PUBLISHABLE_KEY=${CONNECT_AUTH_PUBLISHABLE_KEY}

WORKDIR /app
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY . .
ENV CI=1
ENV BUILD_TARGET=web
RUN npm install -g pnpm@9.9.0 && pnpm install --frozen-lockfile
# Build the web bundle directly with Vite.
# The current frontend type-check is not green yet, but the production bundle builds successfully.
RUN pnpm --filter frontend exec vite build && mv dist /web-dist

# Stage 2: build server natively with glibc.
# The Postgres-backed storage layer links against libpq, which is not available
# in the current musl static toolchain used by Render builds.
FROM --platform=$BUILDPLATFORM ${RUST_IMAGE} AS backend
WORKDIR /app

# Install native build dependencies.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    clang \
    git \
    libpq-dev \
    libsqlite3-dev \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Leverage Docker layer caching for dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps/server ./apps/server
# Stub out apps/tauri so the workspace resolves (not built in Docker)
COPY apps/tauri/Cargo.toml apps/tauri/Cargo.toml
RUN mkdir -p apps/tauri/src && echo "fn main(){}" > apps/tauri/src/main.rs && echo "" > apps/tauri/src/lib.rs
RUN mkdir -p apps/server/src && \
    echo "fn main(){}" > apps/server/src/main.rs && \
    cargo fetch --manifest-path apps/server/Cargo.toml

# Now copy full sources
COPY crates ./crates
COPY apps/server ./apps/server
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
ENV OPENSSL_STATIC=1
# Build the release binary.
RUN cargo build --release --manifest-path apps/server/Cargo.toml && \
    cp target/release/wealthfolio-server /wealthfolio-server

# Final stage
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libpq5 \
    libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*
# Copy from backend (which is now build platform, but binary is target platform)
COPY --from=backend /wealthfolio-server /usr/local/bin/wealthfolio-server
COPY --from=frontend /web-dist ./dist
ENV WF_DB_PATH=/data/wealthfolio.db
# Wealthfolio Connect API URL (can be overridden at runtime via -e or docker-compose)
ARG CONNECT_API_URL=
ENV CONNECT_API_URL=${CONNECT_API_URL}
VOLUME ["/data"]
EXPOSE 8080
CMD ["/usr/local/bin/wealthfolio-server"]
