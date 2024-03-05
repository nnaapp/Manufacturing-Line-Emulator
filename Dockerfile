# Use the latest version of the Rust base image
FROM rust:latest

# Set the working directory in the container to /my
WORKDIR /usr/src/manufacturing

# Expose port
EXPOSE 4855

# Copy Cargo.toml followed by the rest of the Rust project files to the working directory
# Note: Cargo.toml is copied separately to slightly speed up docker build speed and show cargo's dependancy compilation progress
COPY Cargo.toml . ./

# Build the Rust app
RUN cargo build

# Set the command to run the Rust app
CMD cargo run
