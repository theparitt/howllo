FROM rust:1.89-bookworm AS builder

WORKDIR /app/howllo-server

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY howllo-server/Cargo.toml howllo-server/Cargo.lock howllo-server/build.rs ./
COPY howllo-server/src ./src
COPY howllo-server/db ./db
COPY howllo-server/.sqlx ./.sqlx
COPY howllo-web/public/plugins /app/howllo-web/public/plugins

ENV SQLX_OFFLINE=true
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --home /app --shell /usr/sbin/nologin howllo \
    && mkdir -p /app/data/uploads \
    && chown -R howllo:howllo /app/data
COPY --from=builder /app/howllo-server/target/release/howllo-server /usr/local/bin/howllo-server
USER howllo
WORKDIR /app

EXPOSE 7700

CMD ["/usr/local/bin/howllo-server"]
