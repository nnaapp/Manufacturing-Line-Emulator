# Requirements
Latest version of Rust

OpenSSL (Docker makes this easy)

OpenSSL may require the pkg-config package on Linux

Docker handles all local dependencies (Rust, OpenSSL)

# Building and Running
To build only, `cargo build`.\
To run or build and run, `cargo run`.

In the console, it will print the address of the OPC server, as well as the address of the web app control panel. Open the control
panel to control the simulator.

<b>The simulator looks in the project_root/data/ directory for config files, passed by name from the web app control panel.
This is done because the docker container needs to mount a directory to avoid rebuilding every time you change a config file.
If you really want this to change, the two places of interest are servers.rs in the toggleSim POST, as well as main.rs in the
factorySetup function. Otherwise, put configs in that directory and follow the instructions in this README.</b>

# Running with Docker
Run `docker build -t container_name .` in the root directory,
and run with your preferred variant of `docker run --net=host --mount type=bind,source="$(pwd)"/data,target=/home/data -it container_name`.

`--net=host` makes the container use the same network as the host machine, which makes it much easier to connect to the OPC server/control panel.

`--mount type=bind,source="$(pwd)"/data,target=/home/data` mounts ./data to /home/data, which is how the container can on-the-fly see and use 
new config files. Change these two directories if you need to, but you will need to recompile after changing the file reads in code, as stated above.

# JSON Configuration Guide
An example JSON is included (factory.json), but the following is a key of what each field means, organized by scope.

## Factory

- **name**: Name of the line/factory
- **description**: Description of the line/factory
- **simSpeed**: Multiplier for how fast the simulation should run
- **pollRate**: Rate at which the server polls machines in ms

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
The OPC UA Discovery URL will be in the console when the simulator is run. Copy this URL and use it to connect to the client software of choice. 

The control panel link will also be displayed in the console, just open it in your browser of choice.

If you cannot see the console for some reason, they will be similar to the following:\
opc.tcp://your_ip:4855/ \
http://your_ip:8080/

# Contributors
- nnaapp (Connor Burnett)
- coutRun (Seth Thompson)
- MMcCready (Mary McCready)
- rozeng (Robert Zheng)
- Pokemon151 (Joshua Eldridge)
- fivefootbot (Amy McCaughan)
