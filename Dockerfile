# Use the latest version of the Rust base image
FROM rust:latest

# Set the working directory in the container to /my
WORKDIR /usr/src/manufacturing

# Expose port
EXPOSE 4855

# Copy Cargo.toml followed by the rest of the Rust project files to the working directory
# Note: Cargo.toml is copied separately to be cached as its own Docker layer
# As a result, Rust dependancies do not have to be recompiled every time a file is modified
COPY Cargo.toml . ./

# Build the Rust app
RUN cargo build

# Set the command to run the Rust app
CMD cargo run
