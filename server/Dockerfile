# Build from the forme/ repo root:
#   docker build -f server/Dockerfile -t formepdf/forme .

# Pull pre-built rasterizer binary
FROM formepdf/rasterizer:0.14.0 AS rasterizer

FROM rust:1.98-trixie AS builder

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

# --- Runtime stage ---
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*

# Download PDFium binary for the target architecture
ARG TARGETARCH
ARG PDFIUM_VERSION=chromium/8021
RUN set -eux; \
    case "${TARGETARCH:-amd64}" in \
      amd64) PDFIUM_ARCH="linux-x64" ;; \
      arm64) PDFIUM_ARCH="linux-arm64" ;; \
      *) echo "Unsupported arch: $TARGETARCH" && exit 1 ;; \
    esac; \
    curl -sL "https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_VERSION}/pdfium-${PDFIUM_ARCH}.tgz" \
      | tar xzf - -C /usr/local lib/libpdfium.so; \
    ldconfig

COPY --from=builder /app/server/target/release/forme-server /usr/local/bin/forme-server
COPY --from=rasterizer /usr/local/bin/forme-rasterizer /usr/local/bin/forme-rasterizer

RUN useradd -r -s /bin/false forme
USER forme

ENV HTTP_PORT=3000
ENV RASTERIZER_URL=http://localhost:3001
ENV RASTERIZER_PORT=3001
EXPOSE 3000

CMD ["sh", "-c", "forme-rasterizer & exec forme-server"]
