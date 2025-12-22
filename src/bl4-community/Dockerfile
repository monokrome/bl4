FROM rustlang/rust:nightly-bookworm as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p bl4-community

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/bl4-community /usr/local/bin/
EXPOSE 3030
CMD ["bl4-community", "serve", "-b", "0.0.0.0"]
