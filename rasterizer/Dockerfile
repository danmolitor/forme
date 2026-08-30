# Build from the forme/ repo root:
#   docker build -f rasterizer/Dockerfile -t formepdf/rasterizer .
#
# Supports multi-arch: builds for linux/amd64 and linux/arm64.
#
# Alpine (musl) throughout: debian-slim ships perl-base (Essential) which
# carries perpetual unfixed CVEs that tank the Docker Hub security grade.
# Rust musl targets default to static linking, which breaks the dlopen()
# pdfium-render uses to load libpdfium.so — hence -crt-static is disabled.

FROM rust:1.98-alpine3.24 AS builder

RUN apk add --no-cache musl-dev

ENV RUSTFLAGS="-C target-feature=-crt-static"

WORKDIR /app

# Copy manifests for dependency layer caching
COPY rasterizer/Cargo.toml rasterizer/Cargo.lock* rasterizer/

# Create dummy main.rs to pre-build dependencies
RUN mkdir -p rasterizer/src && echo "fn main() {}" > rasterizer/src/main.rs
WORKDIR /app/rasterizer
RUN cargo build --release 2>/dev/null || true

# Copy actual source
COPY rasterizer/src/ src/

# Touch main.rs so cargo rebuilds it (not just the cached dummy)
RUN touch src/main.rs
RUN cargo build --release

# --- PDFium fetch stage (keeps curl out of the runtime image) ---
FROM alpine:3.24 AS pdfium

RUN apk add --no-cache curl

ARG TARGETARCH
ARG PDFIUM_VERSION=chromium/8021
RUN set -eux; \
    case "$TARGETARCH" in \
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
COPY --from=builder /app/rasterizer/target/release/forme-rasterizer /usr/local/bin/forme-rasterizer

RUN adduser -S -D -H -s /sbin/nologin forme
USER forme

ENV RASTERIZER_PORT=3001
EXPOSE 3001

CMD ["forme-rasterizer"]
