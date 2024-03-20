# Requirements
Latest version of Rust

OpenSSL (Docker makes this easy)

OpenSSL may require the pkg-config package on Linux

Docker handles all local dependencies (Rust, OpenSSL)

# Building and Running
To build only, `cargo build --bin simulator --features build-simulator` and/or `cargo build --bin wrapper --features build-wrapper`.

To run or build and run, `cargo run --bin simulator --features build-simulator` and/or `cargo build --bin wrapper --features build-wrapper`

Simulator is just the simulator command line program.

Wrapper is a WIP GUI wrapper that will let you start/stop the simulator, and specify config file location.

# Running with Docker
Run `docker build -t <name> .` in the root directory,
and run with your preferred variant of `docker run --net=host <name>`.

The discovery address of the server will be printed to console, but the IP
will be the same as the host machine, as it runs on the host network.

# Contributors
- nnaapp (Connor Burnett)
- coutRun (Seth Thompson)
- MMcCready (Mary McCready)
- rozeng (Robert Zheng)
- Pokemon151 (Joshua Eldridge)
- Amy McCaughan
