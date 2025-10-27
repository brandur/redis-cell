################################ BUILDER ######################################
FROM rust:1.89-bookworm AS builder

WORKDIR /redis-cell

COPY src ./src
COPY include ./include
COPY Cargo* .
COPY build.rs .

RUN cargo build --release --features valkey

################################ RUNTIME ######################################
FROM valkey/valkey:9.0.0 AS runtime

RUN mkdir -p /usr/local/lib/valkey/modules
COPY --from=builder /redis-cell/target/release/libredis_cell.so /usr/local/lib/valkey/modules/libredis_cell.so
RUN chown -R valkey:valkey /usr/local/lib/valkey

USER valkey

CMD ["valkey-server", "--loadmodule", "/usr/local/lib/valkey/modules/libredis_cell.so"]


