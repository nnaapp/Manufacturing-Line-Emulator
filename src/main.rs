#![allow(non_snake_case)]

mod machine;
use machine::*;

mod json;
use json::*;

mod servers;
use servers::*;

use std::borrow::BorrowMut;
use std::time::Instant;
use std::collections::HashMap;
use std::thread;
use std::cell::{RefCell, RefMut};
use std::sync::Arc;

use opcua::server::prelude::*;
use opcua::sync::RwLock as opcuaRwLock;

use log2::*;

use in_container;

use anyhow::Result;

use chrono::{Datelike, Timelike, Utc};

fn main() -> Result<()>
{
    // Ensure the simulation state is set to running, and initialize the web server for the control panel
    simStateManager(true, Some(SimulationState::STOP));    
    thread::spawn(|| {
        let _ = initWebServer();
    });

    // Set up the OPC UA server and keep track of the address space (Arc<RwLock<AddressSpace>>)
    let opcuaServer = initOPCServer();
    let mut addressSpace = opcuaServer.address_space();
    thread::spawn(|| {
        opcuaServer.run();
    });

    // Loop forever, starting the simulation if it is stopped and the state is "RUNNING"
    // Exit if the signal is given by breaking the loop
    loop
    {
        let state = simStateManager(false, None);
        
        if state == SimulationState::RUNNING
        {
            simClockManager(true, false, None);
            let _ = simulation(&mut addressSpace);
        }
        else if state == SimulationState::EXIT
        {
            break;
        }
    }

    Ok(())
}

// Used to be main, this is the simulation logic that runs until the web server signals it to stop
fn simulation(addressSpace: &mut Arc<opcuaRwLock<AddressSpace>>) -> std::io::Result<()>
{
    let fmtTime = Utc::now();
    let logFileName = format!("{}-{}-{}_{}-{}-{}-log.txt", fmtTime.month(), fmtTime.day(), fmtTime.year(), 
                                fmtTime.hour(), fmtTime.minute(), fmtTime.second());
    let _log2 = log2::open(logFileName.as_str())
        .size(100 * 1024 * 1024) // Maximum file size 100MB
        .rotate(20)
        .tee(true) // Also print to stdout, on top of the file
        .start();

    // Tuple of line data structures and settings
    let factoryDataOption = factorySetup();

    if factoryDataOption.is_none(){
        return Ok(());
    }

    let factoryData = factoryDataOption.unwrap();

    // HashMap<String, RefCell<Machine>> containing all machines
    let mut machines = factoryData.0;
    // Immutable reference vector containing every machine ID
    let machineIDs = factoryData.1;
    // HashMap<String, RefCell<ConveyorBelt>> containing all conveyors
    let mut conveyors = factoryData.2;
    // Immutable reference vector containing every conveyor ID
    let conveyorIDs = factoryData.3;

    //Simulation speed
    let simSpeed: f64 = factoryData.4;
    // Server poll rate in microseconds
    let pollRateUs = factoryData.5;
    let mut pollDeltaTimeUs = 0; // microseconds passed since last poll 

    // Set up the server with the new machine data, and get a Hashmap<String, NodeId> of all nodes
    // on the server
    let nodeIDs = serverSetup(addressSpace, machines.clone(), "MyLine");

    // Time at the instant of beginning the simulation, used to calculate
    // time passage based on the elapsed time from this moment in microseconds
    let start = Instant::now();
    // Two times so we can calculate how long loop iterations took
    let mut iterTime = start.elapsed().as_micros();
    let mut prevTime = iterTime;
    // deltaTime, or dT, is the difference in time between two loop iterations
    let mut deltaTime:u128;


    // In the case the user sets a time limit, this will be used to stop the sim when the time has passed
    let mut executionTimer = 0; // Microseconds counter
    let timerLimit = simTimerManager(false, None) as u128; // Time limit for the sim
    let timerExists = timerLimit != 0; // If this resolves to true, a timer was set

    // Loop until the signal is given to stop this simulation or exit the entire program
    // and perform the simulation logic
    let mut pauseHappened = false;
    while simStateManager(false, None) != SimulationState::STOP && simStateManager(false, None) != SimulationState::EXIT
    {               
        // Time at start of loop    
        iterTime = start.elapsed().as_micros();

        // Microsecond change in time between executions of loop
        deltaTime = ((iterTime - prevTime) as f64 * simSpeed) as u128;

        // Just log that a pause happened and skip all of the simulating if the pause signal is set
        if simStateManager(false, None) == SimulationState::PAUSED 
        {
            // Track time while paused for a total runtime tracker
            simClockManager(false, true, Some(deltaTime));
            // Log loop start time, to calculate difference in time later
            prevTime = iterTime;
            pauseHappened = true; 
            continue; 
        }
        
        // If a pause occurred, we don't want the huge time gap between this iteration
        // and last iteration to cause a speed up in production. Setting dT to 0 fixes this.
        if pauseHappened
        {
            pauseHappened = false;
            deltaTime = 0;
        }   

        // Update runtime clocks with new deltaTime
        simClockManager(false, true, Some(deltaTime));

        // If there is an execution time limit set, check it
        if timerExists
        {
            executionTimer += deltaTime;
            if executionTimer >= timerLimit
            {
                debug!("Execution time exceeded, ending simulation.");
                simStateManager(true, Some(SimulationState::STOP));
                break;
            }
        }

        // For every machine, update its state by checking if it needs to perform
        // any actions, based on the amount of time that has passed.
        // This works on a sort of "tick" system.
        for id in machineIDs.iter()
        {
            machines.get_mut(id)
                    .expect(format!("Machine {id} does not exist.").as_str())
                    .borrow_mut()
                    .get_mut()
                    .update(&mut conveyors, deltaTime);
        }
        
        // For every belt, update its state by checking if it needs to
        // move anything, give anything, or take anything. This also works
        // on a tick system.
        for id in conveyorIDs.iter()
        {
            // Get reference to current conveyor
            let mut conveyor = conveyors.get(id).expect(format!("Conveyor {id} does not exist.").as_str()).borrow_mut();
            // Initialize input conveyor to None by default, as most belts will NOT take input from other belts
            let mut inputConveyor: Option<RefMut<ConveyorBelt>> = None;
            // If the current conveyor has some value for inputID (conveyor to take from),
            // get that conveyor as a reference and make it an option
            if conveyor.isInputIDSome
            {
                let inputID = conveyor.inputID.as_ref().unwrap();
                inputConveyor = Some(conveyors.get(inputID).expect(format!("Conveyor {inputID} does not exist.").as_str()).borrow_mut());
            }
            conveyor.update(inputConveyor, deltaTime);
        }


        // Check if the server should poll for updates
        pollDeltaTimeUs += deltaTime;
        if pollDeltaTimeUs >= pollRateUs
        {
            pollDeltaTimeUs -= pollRateUs;
            let mut addressSpace = addressSpace.write();
            serverPoll(&mut addressSpace, &machines, &nodeIDs, &machineIDs);
        }

        // Log loop start time, to calculate difference in time later
        prevTime = iterTime;
    }

    // If we are outside the main loop, the simulation has ended, so we will wipe the data from the OPC server.
    for nodeID in nodeIDs.values().cloned().collect::<Vec<NodeId>>()
    {
        addressSpace.write().delete(&nodeID, true);
    }

    for id in machineIDs
    {
        let machine = machines.get(&id).expect("Machine ceased to exist.").borrow();
        info!("\nMachine: {}\nConsumed: {}\nProduced: {}\nState Changes: {}\nFaults: {}", 
                machine.id, machine.consumedCount, machine.producedCount, machine.stateChangeCount, machine.faultedCount);
    }
    
    Ok(())
}

fn factorySetup() -> Option<(HashMap<String, RefCell<Machine>>, Vec<String>, 
                        HashMap<String, RefCell<ConveyorBelt>>, Vec<String>, f64, u128)>
{
    let file_path = simConfigManager(false, None);
    let json_data: String;
    if in_container::in_container()
    {
        json_data = read_json_file(format!("/home/data/{}", file_path).as_str());
    } else {
        json_data = read_json_file(format!("./data/{}", file_path).as_str());
    }

    // If we get here, the web service already checked if the JSON has a valid structure
    // using json schemas, so we can parse this without worrying about a panic
    let data: JSONData = serde_json::from_str(&json_data).expect("Failed to parse JSON");


    info!("Factory Name: {}", data.factory.name);
    info!("Description: {}", data.factory.description);
    info!("Simulation Speed: {} ", data.factory.simSpeed);
    info!("Poll Rate: {} milliseconds", data.factory.pollRateMs);

    //Setting data to variables to be passed into the return
    let factorySpeed = data.factory.simSpeed; 
    let factoryPollRateUs = data.factory.pollRateMs * 1000; // milliseconds to microseconds

    let mut machines = HashMap::<String, RefCell<Machine>>::new();
    let mut conveyors = HashMap::<String, RefCell<ConveyorBelt>>::new();
    let mut machineIDs = Vec::<String>::new(); // Track all IDs, this makes iterating over the hashmap easier in the future
    let mut conveyorIDs = Vec::<String>::new();

    for machine in data.factory.machines 
    {
        let mut state = OPCState::PRODUCING;

        match machine.state.to_lowercase().as_str()
        {
            "producing" => state = OPCState::PRODUCING,
            "faulted" => state = OPCState::FAULTED,
            "blocked" => state = OPCState::BLOCKED,
            "starved" => state = OPCState::STARVED,
            _ => (),
        }
        
        let id = String::from(machine.id);

        let mut machineFaults = Vec::<Fault>::new();
        for fault in machine.faults
        {
            machineFaults.push(Fault { faultChance: fault.faultChance, faultMessage: fault.faultMessage, 
                    faultTimeHighSec: fault.faultTimeHighSec, faultTimeLowSec: fault.faultTimeLowSec });
        }
        
        let mut newMachine = Machine::new(
            id.clone(),
            machine.cost,
            machine.throughput,
            state,
            machineFaults,
            data.factory.debounceRateInPolls,
            machine.processingSpeedMs * 1000, // milliseconds to microseconds
            machine.inputSpeedMs * 1000, // milliseconds to microseconds
            machine.inputCapacity,
            machine.outputSpeedMs * 1000, // milliseconds to microseconds
            machine.outputCapacity,
            machine.sensor,
            machine.sensorBaseline,
            machine.sensorVariance,
        );
        newMachine.inputIDs = machine.inputIDs;
        newMachine.outputIDs = machine.outputIDs;

        let mut inputBehavior: fn(&mut Machine, &mut HashMap<String, RefCell<ConveyorBelt>>, u128) -> bool = Machine::singleInput;
        let mut processingBehavior: fn(&mut Machine, u128) -> bool = Machine::defaultProcessing;
        let mut outputBehavior: fn(&mut Machine, &mut HashMap<String, RefCell<ConveyorBelt>>, u128) -> bool = Machine::singleOutput;
        match machine.inputBehavior.to_lowercase().as_str()
        {
            "spawner" => inputBehavior = Machine::spawnerInput,
            "single" => inputBehavior = Machine::singleInput,
            // "flow" => inputBehavior = Machine::flowInput,
            _ => (),
        }
        match machine.processingBehavior.to_lowercase().as_str()
        {
            "default" => processingBehavior = Machine::defaultProcessing,
            // "flow" => processingBehavior = Machine::flowProcessing,
            _ => (),
        }
        match machine.outputBehavior.to_lowercase().as_str()
        {
            "consumer" => outputBehavior = Machine::consumerOutput,
            "default" => outputBehavior = Machine::singleOutput,
            _ => (),
        }

        newMachine.inputBehavior = Some(inputBehavior);
        newMachine.processingBehavior = Some(processingBehavior);
        newMachine.outputBehavior = Some(outputBehavior);

        machineIDs.push(id.clone());
        machines.insert(id.clone(), RefCell::new(newMachine));
    }

    for conveyor in data.factory.conveyors
    {
        let id = String::from(conveyor.id);
        conveyors.insert(id.clone(), RefCell::new(ConveyorBelt::new(id.clone(), conveyor.capacity, conveyor.beltSpeedMs * 1000, conveyor.inputID)));
        conveyorIDs.push(id.clone());
    }

    return Some((machines, machineIDs, conveyors, conveyorIDs, factorySpeed, factoryPollRateUs));
}

// Returns a tuple containing the new Server, as well as a HashMap of machine IDs to OPC NodeIDs
// Set up the OPC server with tags, folders, etc for every machine, and give each machine
// its variables/values to be updated later when the server polls
fn serverSetup(addressSpace: &mut Arc<opcuaRwLock<AddressSpace>>, machinesHashMap: HashMap<String, RefCell<Machine>>, lineName: &str) -> HashMap<String, NodeId>
{
    let machinesHashMap = machinesHashMap.values();
    let mut machines = Vec::<Machine>::new();
    for machine in machinesHashMap
    {
        machines.push(machine.borrow().clone());
    }
    
    let ns = {
        let address_space = addressSpace.clone();
        let mut address_space = address_space.write();
        address_space.register_namespace("urn:line-server").unwrap()
    };

    let mut nodeIDs = HashMap::<String, NodeId>::new();

    {
        let mut addressSpace = addressSpace.write();

        let folderID = addressSpace.add_folder(lineName, lineName, &NodeId::objects_folder_id()).unwrap();
        nodeIDs.insert(String::from("root"), folderID.clone());

        for i in 0 as usize..machines.len()
        {
            let machineID = machines[i].id.to_string();
            // Making folder for machine and its tags, child of line folder
            let machineName = format!("Machine-ID-{machineID}");
            let machineFolderID = addressSpace.add_folder(machineName.clone(), machineName.clone(), &folderID).unwrap();
            
            // Vector of this machine's variable nodes
            let mut variables = Vec::<Variable>::new();

            // Macro to add a new variable to the server, used in the form of:
            // add_server_variable!(variable_name, machine_field, type), 
            // eg. cost_amount, cost (for machine.cost), u64 (to cast the value to u64)
            //
            // This is done as a macro because it's impossible to vary the field of a 
            // struct you add in a function, and this is also the "default case", some
            // server variables deviate from this and are done manually.
            macro_rules! add_server_variable
            {
                ($var_name:expr, $($machine_field:ident).+, $type:ty) => {
                    {
                        let nodeName = $var_name;
                        let nodeID = NodeId::new(ns, format!("{machineID}-{nodeName}"));
                        variables.push(
                            Variable::new(&nodeID, 
                            nodeName,
                            nodeName,
                            machines[i].$($machine_field).+ as $type));
                        nodeIDs.insert(format!("{machineID}-{nodeName}"), nodeID);
                    }
                }
            }
            
            // State node initialization
            // Done without macro for example of what macro does, and due to special case of converting
            // state to string with .to_string()
            let stateVarName = "state";
            let stateNodeID = NodeId::new(ns, format!("{machineID}-state"));
            variables.push(
                Variable::new(&stateNodeID,
                stateVarName, 
                stateVarName, 
                machines[i].state.to_string()));
            nodeIDs.insert(format!("{machineID}-state"), stateNodeID);

            // Fault message node initialization
            // Done without macro due to the fault message being an option, unlike any other field used here
            let faultMsgVarName = "fault-message";
            let faultMsgNodeID = NodeId::new(ns, format!("{machineID}-fault-msg"));
            let mut faultMessage = String::from("");
            if machines[i].currentFault.is_some()
            {
                faultMessage = machines[i].currentFault.clone().expect("Fault does not exist, somehow.").faultMessage;
            }
            variables.push(
                Variable::new(&faultMsgNodeID,
                faultMsgVarName,
                faultMsgVarName,
                faultMessage));
            nodeIDs.insert(format!("{machineID}-fault-msg"), faultMsgNodeID);

            add_server_variable!("produced-count", producedCount, u64);
            add_server_variable!("consumed-count", consumedCount, u64);
            add_server_variable!("state-change-count", stateChangeCount, u64);
            add_server_variable!("fault-count", faultedCount, u64);
            add_server_variable!("input-inventory", inputInventory, u64);
            add_server_variable!("output-inventory", outputInventory, u64);
            if machines[i].sensor == true
            {
                add_server_variable!("sensor", baseline, f64)
            }

            let _ = addressSpace.add_variables(variables, &machineFolderID);
        }
    }

    return nodeIDs;
}

// Handles updating the values of each machine on the OPC server
fn serverPoll(addressSpace: &mut AddressSpace, machines: &HashMap<String, RefCell<Machine>>, nodeIDs: &HashMap<String, NodeId>, ids: &Vec<String>)
{
    let now = DateTime::now();
    // For every machine ID, get that machine and update all of its values on the OPC server
    // as well as provide a current timestamp
    // (maybe we should have this respect simSpeed, in case the user is running at multiplied timescale)
    for id in ids.iter()
    {
        let mut machine = machines.get(id).expect("Machine ceased to exist.").borrow_mut();
        let machineID = machine.id.to_string();

        // This checks for any machine state updates, and it is done here
        // so that we don't have to make a second timer for state debouncing,
        // and can instead measure the time it takes to change a state in
        // server poll cycles
        machine.updateState();

        let stateNodeID = nodeIDs.get(&format!("{machineID}-state")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(stateNodeID, machine.state.to_string(), &now, &now);


        // Macro to update a variable on the server, used in the form of:
        // update_server_variable!(variable_name, machine_field, type), 
        // eg. cost_amount, cost (for machine.cost), u64 (to cast the value to u64)
        //
        // This is done as a macro because it's impossible to vary the field of a 
        // struct you add in a function, and this is also the "default case", some
        // server variables deviate from this and are done manually.
        macro_rules! update_server_variable
        {
            ($var_name:expr, $($machine_field:ident).+, $type:ty) => {
                {
                    let nodeName = $var_name;
                    let nodeID = nodeIDs.get(&format!("{machineID}-{nodeName}")).expect("NodeId ceased to exist.");
                    addressSpace.set_variable_value(nodeID, machine.$($machine_field).+ as $type, &now, &now);
                }
            }
        }

        let faultMsgNodeID = nodeIDs.get(&format!("{machineID}-fault-msg")).expect("NodeId ceased to exist.");
        let mut faultMessage = String::from("");
        if machine.currentFault.is_some()
        {
            faultMessage = machine.currentFault.clone().expect("Fault does not exist, somehow.").faultMessage;
        }
        addressSpace.set_variable_value(faultMsgNodeID, faultMessage, &now, &now);

        update_server_variable!("produced-count", producedCount, u64);
        update_server_variable!("consumed-count", consumedCount, u64);
        update_server_variable!("state-change-count", stateChangeCount, u64);
        update_server_variable!("fault-count", faultedCount, u64);
        update_server_variable!("input-inventory", inputInventory, u64);
        update_server_variable!("output-inventory", outputInventory, u64);

        if machine.sensor == true 
        {
            // Currently not stored in machine, should probably change later
            
            //println!("Machine ID: {}", machine.id);   //here for debugging
            let sensorVal = machine::Machine::sensor_Sim(machine.baseline, machine.variance);
            let sensorNodeID = nodeIDs.get(&format!("{machineID}-sensor")).expect("NodeId ceased to exist.");
            addressSpace.set_variable_value(sensorNodeID, sensorVal as f64, &now, &now);
        }
    }
}
