################################ BUILDER ######################################
FROM rust:1.89-bookworm AS builder

WORKDIR /redis-cell

COPY src ./src
COPY include ./include
COPY Cargo* .
COPY build.rs .

RUN cargo build --release

################################ RUNTIME ######################################
FROM redis:8.2.2-bookworm AS runtime

COPY --from=builder /redis-cell/target/release/libredis_cell.so /usr/local/lib/redis/modules/libredis_cell.so

USER redis

CMD ["redis-server"]

