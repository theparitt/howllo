FROM rust:1.89-bookworm

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY db ./db
COPY .sqlx ./.sqlx
COPY .env.example ./.env.example

RUN cargo build --release

EXPOSE 5110

CMD ["./target/release/howllo-server"]
