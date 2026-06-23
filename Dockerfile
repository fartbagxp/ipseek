FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release --bin ipseek

# Download aas-cidr-ranges IP range files
RUN ./target/release/ipseek update --path ./data/raw --parallel 10

# Download gov-domains ASN prefix CSVs
RUN mkdir -p /tmp/gov-asn && \
    for f in fed-gov-prefixes state-gov-prefixes city-gov-prefixes \
              health-it-prefixes hospitals-prefixes insurance-prefixes \
              pbm-prefixes academic-prefixes; do \
      curl -fsSL \
        "https://raw.githubusercontent.com/fartbagxp/gov-domains/main/data/asn/${f}.csv" \
        -o "/tmp/gov-asn/${f}.csv"; \
    done

# Build database from all sources
RUN ./target/release/ipseek update-local --path ./data/raw --gov-asn-path /tmp/gov-asn

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install CA certificates for HTTPS requests
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ipseek /usr/local/bin
COPY --from=builder /app/db.bin /app/db.bin
ENTRYPOINT ["/usr/local/bin/ipseek", "serve"]
