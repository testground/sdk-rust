FROM rust:1.62-bullseye as builder
WORKDIR /usr/src/sdk-rust

# Cache dependencies between test runs,
# See https://blog.mgattozzi.dev/caching-rust-docker-builds/
# And https://github.com/rust-lang/cargo/issues/2644

RUN mkdir -p ./plan/src/
RUN echo "fn main() { println!(\"If you see this message, you may want to clean up the target directory or the Docker build cache.\") }" > ./plan/src/main.rs
COPY ./plan/Cargo.* ./plan/
RUN cd ./plan/ && cargo build

# Coping each files only needed instead of doing `COPY . .` (all of them) in order to avoid unnecessary compiling.
# (e.g. because of modifiying `manifest.toml`.)
# NOTE: `.dockerignore` is not effective because the build context is not this directory on the test run.
# See https://docs.testground.ai/builder-library/docker-generic#usage
COPY ./plan/src ./plan/src
COPY ./plan/examples ./plan/examples

# This is in order to make sure `main.rs`s mtime timestamp is updated to avoid the dummy `main`
# remaining in the release binary.
# https://github.com/rust-lang/cargo/issues/9598
RUN touch ./plan/src/main.rs

RUN cd ./plan/ && cargo build --example example

FROM debian:bullseye-slim
COPY --from=builder /usr/src/sdk-rust/plan/target/debug/examples/example /usr/local/bin/example
EXPOSE 6060
ENTRYPOINT [ "example"]