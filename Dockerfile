FROM rust:1-bookworm AS build
WORKDIR /app

# 1. Build dependencies only (cached until Cargo.toml/Cargo.lock change)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# 2. Build the real code
COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/ita25_bot /usr/local/bin/ita25_bot
CMD ["ita25_bot"]
