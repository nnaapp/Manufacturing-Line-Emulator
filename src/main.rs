#![allow(non_snake_case)]

mod machine;
use machine::*;

mod json;
use json::*;

mod server;
use server::*;

use std::thread;
use std::time::{UNIX_EPOCH,SystemTime,Duration};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::cell::RefCell;
use std::cell::RefMut;

extern crate rand;
use rand::Rng;

extern crate opcua;
use opcua::server::prelude::*;

extern crate log2;
use log2::*;

fn main() -> std::io::Result<()>
{
    // Debug backtrace info
    // env::set_var("RUST_BACKTRACE", "1");

    // function for testing log2's features
    let _log2 = log2::start();
    // log2Test();

    // Tuple of line data structures and settings
    let factoryData = factorySetup();

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
    // Server poll rate in milliseconds
    let pollRate = factoryData.5;
    let mut pollDeltaTime = 0; // time passed since last poll 

    let runtime = factoryData.6;  // milliseconds needed to pass to stop
    let mut timePassed: u128 = 0; // milliseconds passed 

    // Set up the server and get a tuple containing the Server and a HashMap<usize, NodeId> of all nodes
    let serverData = serverSetup(machines.clone(), "MyLine");
    let server = serverData.0;
    let addressSpace = server.address_space();
    let nodeIDs = serverData.1;
    // Spawn a thread for the server to run on independently
    thread::spawn(|| {
        server.run();
    });
    
    // Master random number generator, which is passed to machines to use for faults
    let mut rng = rand::thread_rng();

    // Start represents current SystemTime, 
    // iter/prevTime represent milliseconds since epoch time for the current and previous iteration of loop,
    // deltaTime represents milliseconds time between previous and current iteration of loop.
    let mut start = SystemTime::now();
    let mut iterTime:Duration = start.duration_since(UNIX_EPOCH).expect("Failure while getting epoch time in microseconds");
    let mut prevTime:Duration = iterTime;
    let mut deltaTime:u128;

    let mut dtPeak: u128 = 0;
    let mut dtSum: u128 = 0;
    let mut dtAmount: u128 = 0;

    while timePassed < runtime
    {   
        // Find deltatime between loop iterations
        start = SystemTime::now();
        iterTime = start.duration_since(UNIX_EPOCH).expect("Failure while getting epoch time in microseconds");     
        deltaTime = iterTime.as_micros() - prevTime.as_micros();

        if deltaTime > dtPeak { dtPeak = deltaTime; }
        dtSum += deltaTime;
        dtAmount += 1;
        
        timePassed += deltaTime;
        deltaTime = ((iterTime.as_micros() as f64 * simSpeed) as u128) - (((prevTime.as_micros() as f64) * simSpeed) as u128);

        // rng is used to seed the update with any random integer, which is used for any rng dependent operations
        // update all machines
        for id in machineIDs.iter()
        {
            machines.get_mut(id)
                    .expect(format!("Machine {id} does not exist.").as_str())
                    .borrow_mut()
                    .update(&mut conveyors, deltaTime, rng.gen_range(0..=std::i32::MAX));
        }
        // update all conveyor belts
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
        pollDeltaTime += deltaTime;
        if pollDeltaTime >= pollRate
        {
            pollDeltaTime -= pollRate;
            let mut addressSpace = addressSpace.write();
            serverPoll(&mut addressSpace, &machines, &nodeIDs, &machineIDs);
        }

        // Log system time at the start of this iteration, for use in next iteration
        prevTime = iterTime;
    }
    let file = File::create("log.txt")?;
    writeln!(&file, "Avg Cycle Time: {}", dtSum / dtAmount)?;
    writeln!(&file, "Peak Cycle Time: {}", dtPeak)?;
    writeln!(&file, "")?;
    for id in machineIDs.iter()
    {
        let machine = machines.get(id).expect("Machine ceased to exist.").borrow();
        let efficiencyCount = machine.producedCount as f64 / (machine.throughput as f64 * (runtime as f64 / machine.processingTickSpeed as f64));

        writeln!(&file, "Machine ID: {}", machines.get(id).expect("Machine ceased to exist").borrow().id)?;
        writeln!(&file, "Machine Input: {}", machines.get(id).expect("Machine ceased to exist").borrow().consumedCount)?;
        writeln!(&file, "Machine Output: {}", machines.get(id).expect("Machine ceased to exist").borrow().producedCount)?;
        writeln!(&file, "State Changes: {}", machines.get(id).expect("Machine ceased to exist").borrow().stateChangeCount)?;
        writeln!(&file, "Efficiency: {}", efficiencyCount)?;
        writeln!(&file, "")?;

    }
    Ok(())
}

// fn log2Test()
// {
//     // Start log2
//     let _log2 = log2::start();

//     trace!("Trace Test");
//     debug!("Debug Test");
//     info!("Info Test");
//     warn!("Warn Test");
//     error!("Error Test");
// }

fn factorySetup() -> (HashMap<String, RefCell<Machine>>, Vec<String>, 
                        HashMap<String, RefCell<ConveyorBelt>>, Vec<String>, f64, u128, u128)
{
    let file_path = "factory.json";
    let json_data = read_json_file(file_path);
    let data: JSONData = serde_json::from_str(&json_data).expect("Failed to parse JSON");

    info!("Factory Name: {}", data.factory.name);
    info!("Description: {}", data.factory.description);
    info!("simSpeed: {} ", data.factory.simSpeed);
    info!("pollRate: {} milliseconds", data.factory.pollRate);
    info!("Runtime: {} seconds", data.factory.Runtime);
    info!("");

    //Setting data to variables to be passed into the return
    let factorySpeed = data.factory.simSpeed; 
    let factoryPollRate = data.factory.pollRate * 1000; // milliseconds to microseconds
    let factoryRuntime = data.factory.Runtime * 1000 * 1000; // seconds to microseconds

    let mut machines = HashMap::<String, RefCell<Machine>>::new();
    let mut conveyors = HashMap::<String, RefCell<ConveyorBelt>>::new();
    let mut machineIDs = Vec::<String>::new(); // Track all IDs, this makes iterating over the hashmap easier in the future
    let mut conveyorIDs = Vec::<String>::new();

    for machine in data.factory.Machines 
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
        let mut newMachine = Machine::new(
            id.clone(),
            machine.cost,
            machine.throughput,
            state,
            machine.faultChance,
            machine.faultMessage,
            machine.faultTimeHigh,
            machine.faultTimeLow,
            machine.processingSpeed * 1000, // milliseconds to microseconds
            machine.inputSpeed * 1000, // milliseconds to microseconds
            machine.inputCapacity,
            machine.outputSpeed * 1000, // milliseconds to microseconds
            machine.outputCapacity
        );
        newMachine.inputIDs = machine.inputIDs;
        newMachine.outputIDs = machine.outputIDs;

        let mut inputBehavior: fn(&mut Machine, &mut HashMap<String, RefCell<ConveyorBelt>>, u128) -> bool = Machine::singleInput;
        let mut processingBehavior: fn(&mut Machine, u128, i32) -> bool = Machine::defaultProcessing;
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

    for conveyor in data.factory.Conveyors
    {
        let id = String::from(conveyor.id);
        conveyors.insert(id.clone(), RefCell::new(ConveyorBelt::new(id.clone(), conveyor.capacity, conveyor.beltSpeed * 1000, conveyor.inputID)));
        conveyorIDs.push(id.clone());
    }

    return (machines, machineIDs, conveyors, conveyorIDs, factorySpeed, factoryPollRate, factoryRuntime);
}

// Returns a tuple containing the new Server, as well as a HashMap of machine IDs to OPC NodeIDs
fn serverSetup(machinesHashMap: HashMap<String, RefCell<Machine>>, lineName: &str) -> (Server, HashMap<String, NodeId>)
{
    let machinesHashMap = machinesHashMap.values();
    let mut machines = Vec::<Machine>::new();
    for machine in machinesHashMap
    {
        machines.push(machine.borrow().clone());
    }
    
    let server = initServer();

    let ns = {
        let address_space = server.address_space();
        let mut address_space = address_space.write();
        address_space.register_namespace("urn:line-server").unwrap()
    };

    let mut nodeIDs = HashMap::<String, NodeId>::new();

    let addressSpace = server.address_space();

    {
        let mut addressSpace = addressSpace.write();

        let folderID = addressSpace.add_folder(lineName, lineName, &NodeId::objects_folder_id()).unwrap();

        for i in 0 as usize..machines.len()
        {
            let machineID = machines[i].id.to_string();
            // Making folder for machine and its tags, child of line folder
            let machineName = format!("Machine-ID-{machineID}");
            let machineFolderID = addressSpace.add_folder(machineName.clone(), machineName.clone(), &folderID).unwrap();
            
            // Vector of this machine's variable nodes
            let mut variables = Vec::<Variable>::new();
            
            // State node initialization
            let stateVarName = "state";
            let stateNodeID = NodeId::new(ns, format!("{machineID}-state"));
            variables.push(
                Variable::new(&stateNodeID,
                stateVarName, 
                stateVarName, 
                machines[i].state.to_string()));
            nodeIDs.insert(format!("{machineID}-state"), stateNodeID);

            let faultMsgVarName = "fault-message";
            let faultMsgNodeID = NodeId::new(ns, format!("{machineID}-fault-msg"));
            variables.push(
                Variable::new(&faultMsgNodeID,
                faultMsgVarName,
                faultMsgVarName,
                machines[i].faultMessage.clone()));
            nodeIDs.insert(format!("{machineID}-fault-msg"), faultMsgNodeID);

            let producedCountVarName = "produced-count";
            let producedCountNodeID = NodeId::new(ns, format!("{machineID}-produced-count"));
            variables.push(
                Variable::new(&producedCountNodeID, 
                producedCountVarName,
                producedCountVarName,
                machines[i].producedCount as u64));
            nodeIDs.insert(format!("{machineID}-produced-count"), producedCountNodeID);

            let inputInventoryVarName = "input-inventory";
            let inputInventoryNodeID = NodeId::new(ns, format!("{machineID}-input-inventory"));
            variables.push(
                Variable::new(&inputInventoryNodeID, 
                inputInventoryVarName,
                inputInventoryVarName,
                machines[i].inputInventory as u64));
            nodeIDs.insert(format!("{machineID}-input-inventory"), inputInventoryNodeID);

            let outputInventoryVarName = "output-inventory";
            let outputInventoryNodeID = NodeId::new(ns, format!("{machineID}-output-inventory"));
            variables.push(
                Variable::new(&outputInventoryNodeID, 
                outputInventoryVarName,
                outputInventoryVarName,
                machines[i].outputInventory as u64));
            nodeIDs.insert(format!("{machineID}-output-inventory"), outputInventoryNodeID);
            
            let _ = addressSpace.add_variables(variables, &machineFolderID);
        }
    }

    return (server, nodeIDs);
}

// Handles updating the values of each machine on the OPC server
fn serverPoll(addressSpace: &mut AddressSpace, machines: &HashMap<String, RefCell<Machine>>, nodeIDs: &HashMap<String, NodeId>, ids: &Vec<String>)
{
    //for machine in machines
    //{
       //call sensorfunction to be updated with serverPoll 
    //}

    let now = DateTime::now();
    for id in ids.iter()
    {
        let machine = machines.get(id).expect("Machine ceased to exist.").borrow();
        let machineID = machine.id.to_string();

        let stateNodeID = nodeIDs.get(&format!("{machineID}-state")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(stateNodeID, machine.state.to_string(), &now, &now);

        let faultMsgNodeID = nodeIDs.get(&format!("{machineID}-fault-msg")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(faultMsgNodeID, machine.faultMessage.clone(), &now, &now);

        let producedCountNodeID = nodeIDs.get(&format!("{machineID}-produced-count")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(producedCountNodeID, machine.producedCount as u64, &now, &now);

        let inputInventoryNodeID = nodeIDs.get(&format!("{machineID}-input-inventory")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(inputInventoryNodeID, machine.inputInventory as u64, &now, &now);

        let outputInventoryNodeID = nodeIDs.get(&format!("{machineID}-output-inventory")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(outputInventoryNodeID, machine.outputInventory as u64, &now, &now);
    }
}
