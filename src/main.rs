#![allow(non_snake_case)]

mod machine;
use machine::*;

mod json;
use json::*;

mod server;
use server::*;

use std::borrow::BorrowMut;
use std::time::{UNIX_EPOCH,SystemTime,Duration};
use std::collections::HashMap;
use std::thread;
use std::cell::{RefCell, RefMut};
use std::sync::{RwLock, Arc};

use opcua::server::prelude::*;
use opcua::sync::RwLock as opcuaRwLock;

use actix_web::{get, post, App, HttpResponse, HttpServer, Responder};

use log2::*;

use anyhow::Result;

fn main() -> Result<()>
{
    let _log2 = log2::start();

    // Ensure the simulation state is set to running, and initialize the web server for the control panel
    simStateManager(true, Some(SimulationState::RUNNING));    
    thread::spawn(|| {
        let _ = webServer();
    });

    // Set up the OPC UA server and keep track of the address space (Arc<RwLock<AddressSpace>>)
    let opcuaServer = initServer();
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
    // Server poll rate in microseconds
    let pollRateUs = factoryData.5;
    let mut pollDeltaTimeUs = 0; // microseconds passed since last poll 

    // Set up the server with the new machine data, and get a Hashmap<String, NodeId> of all nodes
    // on the server
    let nodeIDs = serverSetup(addressSpace, machines.clone(), "MyLine");
    
    // Start represents current SystemTime, 
    // iter/prevTime represent milliseconds since epoch time for the current and previous iteration of loop,
    // deltaTime represents milliseconds time between previous and current iteration of loop.
    let mut start = SystemTime::now();
    let mut iterTime:Duration = start.duration_since(UNIX_EPOCH).expect("Failure while getting epoch time in microseconds");
    let mut prevTime:Duration = iterTime;
    let mut deltaTime:u128;

    // let mut dtPeak: u128 = 0;
    // let mut dtSum: u128 = 0;
    // let mut dtAmount: u128 = 0;

    // Loop until the signal is given to stop this simulation or exit the entire program
    // and perform the simulation logic
    let mut pauseHappened = false;
    while simStateManager(false, None) != SimulationState::STOP && simStateManager(false, None) != SimulationState::EXIT
    {   
        // Just log that a pause happened and skip all of the simulating if the pause signal is set
        if simStateManager(false, None) == SimulationState::PAUSED 
        {
            pauseHappened = true; 
            continue; 
        }
        
        // Find deltatime between loop iterations
        start = SystemTime::now();
        iterTime = start.duration_since(UNIX_EPOCH).expect("Failure while getting epoch time in microseconds");  
        // If a pause occurred, we don't want the huge time gap between this iteration
        // and last iteration to cause a speed up in production. Setting the two times 
        // equal to each other mitigates that, making it like no time passed.
        if pauseHappened
        {
            pauseHappened = false;
            prevTime = iterTime;
        }   
        deltaTime = iterTime.as_micros() - prevTime.as_micros();

        // if deltaTime > dtPeak { dtPeak = deltaTime; }
        // dtSum += deltaTime;
        // dtAmount += 1;
        
        deltaTime = ((iterTime.as_micros() as f64 * simSpeed) as u128) - (((prevTime.as_micros() as f64) * simSpeed) as u128);

        // rng is used to seed the update with any random integer, which is used for any rng dependent operations
        // update all machines
        for id in machineIDs.iter()
        {
            machines.get_mut(id)
                    .expect(format!("Machine {id} does not exist.").as_str())
                    .borrow_mut()
                    .get_mut()
                    .update(&mut conveyors, deltaTime);
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
        pollDeltaTimeUs += deltaTime;
        if pollDeltaTimeUs >= pollRateUs
        {
            pollDeltaTimeUs -= pollRateUs;
            let mut addressSpace = addressSpace.write();
            serverPoll(&mut addressSpace, &machines, &nodeIDs, &machineIDs);
            let state = simStateManager(false, None);
        }

        // Log system time at the start of this iteration, for use in next iteration
        prevTime = iterTime;
    }
    // TODO: make sure this works with new web controller system
    // let file = File::create("log.txt")?;
    // writeln!(&file, "Avg Cycle Time: {}", dtSum / dtAmount)?;
    // writeln!(&file, "Peak Cycle Time: {}", dtPeak)?;
    // writeln!(&file, "")?;
    // for id in machineIDs.iter()
    // {
    //     let machine = machines.get(id).expect("Machine ceased to exist.").borrow();
    //     let efficiencyCount = machine.producedCount as f64 / (machine.throughput as f64 * (runtimeUs as f64 / machine.processingTickSpeedUs as f64));

    //     writeln!(&file, "Machine ID: {}", machines.get(id).expect("Machine ceased to exist").borrow().id)?;
    //     writeln!(&file, "Machine Input: {}", machines.get(id).expect("Machine ceased to exist").borrow().consumedCount)?;
    //     writeln!(&file, "Machine Output: {}", machines.get(id).expect("Machine ceased to exist").borrow().producedCount)?;
    //     writeln!(&file, "State Changes: {}", machines.get(id).expect("Machine ceased to exist").borrow().stateChangeCount)?;
    //     writeln!(&file, "Efficiency: {}", efficiencyCount)?;
    //     writeln!(&file, "")?;

    // }

    for nodeID in nodeIDs.values().cloned().collect::<Vec<NodeId>>()
    {
        addressSpace.write().delete(&nodeID, true);
    }
    
    Ok(())
}

// The four states for the simulation
#[derive(PartialEq, Eq, Clone, Copy)]
enum SimulationState
{
    RUNNING, // Running as normal
    PAUSED,  // Paused but still waiting
    STOP,    // Full-stop the simulation
    EXIT,    // Fully exit the program
}

// This is our solution to getting signals from our Actix HTTP server out into the simulator.
// We could not get an object or other easy to move flag inside the service that the web page uses,
// so we use static memory to keep track of a "master state" for the simulation, which is 
// either get or set depending on function arguments.
// 
// false and None for getter, true and Some(SimulationState::StateHere) for setter 
fn simStateManager(updateState: bool, newState: Option<SimulationState>) -> SimulationState
{
    static STATE: RwLock<SimulationState> = RwLock::new(SimulationState::RUNNING);

    if updateState && newState.is_some()
    {
        *STATE.write().unwrap() = newState.unwrap();
    }

    let state = *STATE.read().ok().unwrap();
    return state.clone();
}

// Get HTML for web page
#[get("/")]
async fn getPage() -> impl Responder
{
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../data/index.html"))
}

// Stop or start the simulation but not the program
#[post("/toggleSim")]
async fn toggleSim() -> impl Responder 
{
    match simStateManager(false, None)
    {
        SimulationState::RUNNING => simStateManager(true, Some(SimulationState::STOP)),
        SimulationState::STOP => simStateManager(true, Some(SimulationState::RUNNING)),
        SimulationState::EXIT => SimulationState::EXIT,
        _ => simStateManager(true, Some(SimulationState::STOP))
    };

    HttpResponse::Ok()
}

// Exit the program entirely
#[post("/exitSim")]
async fn exitSim() -> impl Responder
{
    simStateManager(true, Some(SimulationState::EXIT));

    HttpResponse::Ok()
}

// Pause or unpause the simulation without killing it fully
#[post("/suspendSim")]
async fn suspendSim() -> impl Responder
{
    match simStateManager(false, None)
    {
        SimulationState::RUNNING => simStateManager(true, Some(SimulationState::PAUSED)),
        SimulationState::PAUSED => simStateManager(true, Some(SimulationState::RUNNING)),
        SimulationState::EXIT => SimulationState::EXIT,
        SimulationState::STOP => SimulationState::STOP
    };

    HttpResponse::Ok()
}

// Set up and asynchronously run the Actix HTTP server for the control panel
#[actix_web::main]
async fn webServer() -> std::io::Result<()>
{
    HttpServer::new(|| {
        App::new()
            .service(getPage)
            .service(toggleSim)
            .service(exitSim)
            .service(suspendSim)
        })
        .disable_signals()
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}

fn factorySetup() -> (HashMap<String, RefCell<Machine>>, Vec<String>, 
                        HashMap<String, RefCell<ConveyorBelt>>, Vec<String>, f64, u128, u128)
{
    let file_path = "data/factory.json";
    let json_data = read_json_file(file_path);
    let data: JSONData = serde_json::from_str(&json_data).expect("Failed to parse JSON");

    info!("Factory Name: {}", data.factory.name);
    info!("Description: {}", data.factory.description);
    info!("Simulation Speed: {} ", data.factory.simSpeed);
    info!("Poll Rate: {} milliseconds", data.factory.pollRateMs);
    info!("Runtime: {} seconds", data.factory.runtimeSec);

    //Setting data to variables to be passed into the return
    let factorySpeed = data.factory.simSpeed; 
    let factoryPollRateUs = data.factory.pollRateMs * 1000; // milliseconds to microseconds
    let factoryRuntimeUs = data.factory.runtimeSec * 1000 * 1000; // seconds to microseconds

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

    return (machines, machineIDs, conveyors, conveyorIDs, factorySpeed, factoryPollRateUs, factoryRuntimeUs);
}

// Returns a tuple containing the new Server, as well as a HashMap of machine IDs to OPC NodeIDs
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

            if machines[i].sensor == true
            {
                let sensorVarName = "sensor";
                let sensorNodeID = NodeId::new(ns, format!("{machineID}-sensor"));
                variables.push(
                    Variable::new(&sensorNodeID, 
                    sensorVarName,
                    sensorVarName,
                    machines[i].baseline as f64));
                nodeIDs.insert(format!("{machineID}-sensor"), sensorNodeID);
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
    for id in ids.iter()
    {
        let mut machine = machines.get(id).expect("Machine ceased to exist.").borrow_mut();
        let machineID = machine.id.to_string();

        machine.updateState();

        let stateNodeID = nodeIDs.get(&format!("{machineID}-state")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(stateNodeID, machine.state.to_string(), &now, &now);

        let faultMsgNodeID = nodeIDs.get(&format!("{machineID}-fault-msg")).expect("NodeId ceased to exist.");
        let mut faultMessage = String::from("");
        if machine.currentFault.is_some()
        {
            faultMessage = machine.currentFault.clone().expect("Fault does not exist, somehow.").faultMessage;
        }
        addressSpace.set_variable_value(faultMsgNodeID, faultMessage, &now, &now);

        let producedCountNodeID = nodeIDs.get(&format!("{machineID}-produced-count")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(producedCountNodeID, machine.producedCount as u64, &now, &now);

        let inputInventoryNodeID = nodeIDs.get(&format!("{machineID}-input-inventory")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(inputInventoryNodeID, machine.inputInventory as u64, &now, &now);

        let outputInventoryNodeID = nodeIDs.get(&format!("{machineID}-output-inventory")).expect("NodeId ceased to exist.");
        addressSpace.set_variable_value(outputInventoryNodeID, machine.outputInventory as u64, &now, &now);

        if machine.sensor == true 
        {
            //currently iterates too much but putting it in this loop was the only way I could find to make it work alongside 
            //the baseline, variance, and sensor variables of machines
            
            //println!("Machine ID: {}", machine.id);   //here for debugging
            let sensorVal = machine::Machine::sensor_Sim(machine.baseline, machine.variance);
            let sensorNodeID = nodeIDs.get(&format!("{machineID}-sensor")).expect("NodeId ceased to exist.");
            addressSpace.set_variable_value(sensorNodeID, sensorVal as f64, &now, &now);
        }
    }
}
