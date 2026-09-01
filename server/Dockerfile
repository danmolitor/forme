# Build from the forme/ repo root:
#   docker build -f server/Dockerfile -t formepdf/forme .
#
# Alpine (musl) throughout — see rasterizer/Dockerfile for the rationale.
# The rasterizer binary copied below is musl-linked, so the rasterizer image
# MUST be alpine-based (publish it first; keep the bases in lockstep).

# Pull pre-built rasterizer binary
FROM formepdf/rasterizer:0.14.0 AS rasterizer

FROM rust:1.98-alpine3.24 AS builder

RUN apk add --no-cache musl-dev

ENV RUSTFLAGS="-C target-feature=-crt-static"

WORKDIR /app

# Copy the engine (path dependency)
COPY engine/ engine/

# Copy server manifests for dependency layer caching
COPY server/Cargo.toml server/Cargo.lock server/

# Create dummy main.rs to pre-build dependencies
RUN mkdir -p server/src && echo "fn main() {}" > server/src/main.rs
WORKDIR /app/server
RUN cargo build --release 2>/dev/null || true

# Copy actual source
COPY server/src/ src/

# Touch main.rs so cargo rebuilds it (not just the cached dummy)
RUN touch src/main.rs
RUN cargo build --release

# --- PDFium fetch stage (keeps curl out of the runtime image) ---
FROM alpine:3.24 AS pdfium

RUN apk add --no-cache curl

ARG TARGETARCH
ARG PDFIUM_VERSION=chromium/8021
RUN set -eux; \
    case "${TARGETARCH:-amd64}" in \
      amd64) PDFIUM_ARCH="linux-musl-x64" ;; \
      arm64) PDFIUM_ARCH="linux-musl-arm64" ;; \
      *) echo "Unsupported arch: $TARGETARCH" && exit 1 ;; \
    esac; \
    mkdir -p /pdfium; \
    curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_VERSION}/pdfium-${PDFIUM_ARCH}.tgz" \
      | tar xzf - -C /pdfium lib/libpdfium.so

# --- Runtime stage ---
FROM alpine:3.24

# apk upgrade picks up security fixes newer than the base image snapshot
# (e.g. libcrypto3/libssl3). The CA bundle ships in the base via
# ca-certificates-bundle — the full ca-certificates package isn't needed
# and would drag in the openssl CLI.
RUN apk upgrade --no-cache && apk add --no-cache libstdc++

# musl's default library search path includes /usr/local/lib — no ldconfig
COPY --from=pdfium /pdfium/lib/libpdfium.so /usr/local/lib/libpdfium.so
COPY --from=builder /app/server/target/release/forme-server /usr/local/bin/forme-server
COPY --from=rasterizer /usr/local/bin/forme-rasterizer /usr/local/bin/forme-rasterizer

RUN adduser -S -D -H -s /sbin/nologin forme
USER forme

ENV HTTP_PORT=3000
ENV RASTERIZER_URL=http://localhost:3001
ENV RASTERIZER_PORT=3001
EXPOSE 3000

CMD ["sh", "-c", "forme-rasterizer & exec forme-server"]
