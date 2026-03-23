# 1. Planner Stage: Prepare the recipe
FROM lukemathwalker/cargo-chef:latest-rust-1.75 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# 2. Builder Stage: Build the dependencies
FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# This layer is cached until your Cargo.toml changes!
RUN cargo chef cook --release --recipe-path recipe.json

# 3. Final Build: Compile your actual code
COPY . .
RUN cargo build --release --workspace

# 4. Runtime Stage: The "Sovereign" Binary
FROM debian:bookworm-slim AS runtime
WORKDIR /app
# Copy only the binaries you need (CLI and Converter)
COPY --from=builder /app/target/release/malgam-cli /usr/local/bin/
COPY --from=builder /app/target/release/malgam-convert /usr/local/bin/

ENTRYPOINT ["malgam-cli"]