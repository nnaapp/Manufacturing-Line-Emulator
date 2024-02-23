# Requirements
Latest version of Rust
OpenSSL (Docker makes this easy)
OpenSSL may require the pkg-config package on Linux

# Building and Running
Run `cargo build`, and then `cargo run`

# Running with Docker
Run `docker build -t <name> .` in the root directory,
and run with your preferred variant of `docker run --net=host <name>`.

The discovery address of the server will be printed to console, but the IP
will be the same as the host machine, as it runs on the host network.
