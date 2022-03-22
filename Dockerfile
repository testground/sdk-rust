FROM rust:1.57-bullseye as builder
WORKDIR /usr/src/sdk-rust
COPY . .
RUN cd plan && cargo build --example example

FROM debian:bullseye-slim
COPY --from=builder /usr/src/sdk-rust/plan/target/debug/examples/example /usr/local/bin/example
EXPOSE 6060
ENTRYPOINT [ "example"]