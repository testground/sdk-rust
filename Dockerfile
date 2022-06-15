FROM rust:1.57-bullseye as builder
WORKDIR /usr/src/sdk-rust

# Cache dependencies between test runs,
# See https://blog.mgattozzi.dev/caching-rust-docker-builds/
# And https://github.com/rust-lang/cargo/issues/2644

RUN mkdir -p ./plan/src/
RUN echo "fn main() {}" > ./plan/src/main.rs
COPY ./plan/Cargo.lock ./plan/
COPY ./plan/Cargo.toml ./plan/
RUN cd ./plan/ && cargo build

COPY . .
RUN cd ./plan/ && cargo build --example example

FROM debian:bullseye-slim
COPY --from=builder /usr/src/sdk-rust/plan/target/debug/examples/example /usr/local/bin/example
EXPOSE 6060
ENTRYPOINT [ "example"]