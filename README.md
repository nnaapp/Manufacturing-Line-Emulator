# Requirements
Latest version of Rust

OpenSSL (Docker makes this easy)

OpenSSL may require the pkg-config package on Linux

Docker handles all local dependencies (Rust, OpenSSL)

# Building and Running
To build only, `cargo build --bin simulator --features build-simulator`\
and/or `cargo build --bin wrapper --features build-wrapper`.

To run or build and run, `cargo run --bin simulator --features build-simulator`\
and/or `cargo build --bin wrapper --features build-wrapper`

Simulator is just the simulator command line program.

Wrapper is a WIP GUI wrapper that will let you start/stop the simulator, and specify config file location.

# Running with Docker
Run `docker build -t <name> .` in the root directory,
and run with your preferred variant of `docker run --net=host <name>`.

The discovery address of the server will be printed to console, but the IP
will be the same as the host machine, as it runs on the host network.

# JSON Configuration Guide
An example JSON is included (factory.json), but the following is a key of what each field means, organized by scope.

## Factory

- **name**: Name of the line/factory
- **description**: Description of the line/factory
- **simSpeed**: Multiplier for how fast the simulation should run
- **pollRate**: Rate at which the server polls machines in ms
- **Runtime**: Amount of time the simulation will run for, in seconds

## Machines

Machines is an array, each element has the following:

- **id**: String ID of the machine
- **cost**: Amount of input it needs to produce
- **throughput**: Amount of output it produces
- **state**: "PRODUCING", "BLOCKED", "STARVED", or "FAULTED"
- **faultChance**: 0.0 through 1.0 chance of faulting when it produces
- **faultMessage**: String message for when the machine faults
- **faultTimeHigh**: Highest time the machine can stay faulted for
- **faultTimeLow**: Lowest time the machine can stay faulted for
- **inputIDs**: Array of strings, which represent conveyor belt IDs
- **inputBehavior**: "SPAWNER" or "DEFAULT", spawner has infinite supply
- **inputSpeed**: Rate the machine takes input at, in ms, 0 for instant
- **inputCapacity**: How much input the machine can hold at once
- **processingBehavior**: "DEFAULT" only for now
- **processingSpeed**: Rate the machine produces at, in ms, 0 for instant
- **outputIDs**: Array of strings, which represent conveyor belt IDs
- **outputBehavior**: "CONSUMER" or "DEFAULT", consumer has infinite space
- **outputSpeed**: Rate the machine gives output at, in ms, 0 for instant
- **outputCapacity**: How much output the machine can hold at once
- **sensor**: Boolean true/false, determines if the machine has a sensor, sensor is a generic fluctuating value to simulate a variety of real sensors
- **baseline**: The "home" value of the sensor, which it fluctuates around
- **variance**: The maximum distance the sensor can vary from the baseline

## Conveyors

Conveyors is an array, each element has the following:

- **id**: String ID the conveyor belt
- **capacity**: How many items the belt can hold
- **beltSpeed**: Rate at which items move one space on the belt, in ms
- **inputID**: Used for connectinb a belt to another belt, null for none, or conveyor belt ID to connect a belt

# Connecting 
The Discovery URL will be in the command line when the simulator is run. Copy this URL and use it to connect to the client software of choice. 

# Contributors
- nnaapp (Connor Burnett)
- coutRun (Seth Thompson)
- MMcCready (Mary McCready)
- rozeng (Robert Zheng)
- Pokemon151 (Joshua Eldridge)
- fivefootbot (Amy McCaughan)
