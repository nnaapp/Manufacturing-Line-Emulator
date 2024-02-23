#![allow(non_snake_case)]

use std::fmt;
use std::thread;
use std::time::{UNIX_EPOCH,SystemTime,Duration};
use std::collections::HashMap;

extern crate rand;
use rand::Rng;

extern crate serde_json;
extern crate serde;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::{Write, Read};

extern crate opcua;
use std::path::PathBuf;
use opcua::server::prelude::*;

extern crate local_ip_address;
use local_ip_address::local_ip;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OPCState
{
    PRODUCING,
    FAULTED,
    BLOCKED,
    STARVED,
}
impl fmt::Display for OPCState
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self
        {
            OPCState::PRODUCING => write!(f, "producing"),
            OPCState::FAULTED => write!(f, "faulted"),
            OPCState::BLOCKED => write!(f, "blocked"),
            OPCState::STARVED => write!(f, "starved"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct MachineLaneID
{
    machineID: usize,
    laneID: usize,
}

#[derive(Clone)]
struct Machine
{
    id: usize,
    cost: usize, // Cost to produce
    throughput: usize, // How much gets produced
    state: OPCState,
    faultChance: f32,
    faultMessage: String, //string for fault messages 

    processingBehavior: Option<fn(&mut Machine, i32) -> bool>, 
    processingClock: u128, // deltaTime is in milliseconds
    processingTickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    processingTickHeld: bool,

    inputBehavior: Option<fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool>, // Function pointer that can also be None, used to define behavior
    inputClock: u128,
    inputTickSpeed: u128, // tick for pulling input into inputInventory
    inputTickHeld: bool,
    inputIDs: Vec<MachineLaneID>, // Vector of machine/lane IDs for input, used as indices
    inputInventory: usize, // storage place in machine before process 
    inputInvCapacity: usize, 
    nextInput: usize, // the input lane to start checking from 
    
    outputBehavior: Option<fn(&mut Machine) -> bool>,
    outputClock: u128, 
    outputTickSpeed: u128, // tick for outputting 
    outputTickHeld: bool,
    outputInventory: usize, // represents num of items in it 
    outputInvCapacity: usize,
    nextOutput: usize, // the output lane to start checkng from

    outputLanes: usize, // number out output lanes
    beltCapacity: usize, // Capacity of EACH beltInventories
    beltInventories: Vec<usize>, // Vector of inventories, one per output lane

    producedCount: usize,
    consumedCount: usize,
    stateChangeCount: usize,
}
impl Machine
{
    fn new(id: usize, cost: usize, throughput: usize, state: OPCState, faultChance: f32, faultMessage: String,
            processingTickSpeed: u128, inputTickSpeed: u128, inputLanes: usize, inputInvCapacity: usize,
            outputTickSpeed: u128, outputInvCapacity: usize, outputLanes: usize, beltCapacity: usize) -> Self
    {
        let mut inIDs = Vec::<MachineLaneID>::new();
        inIDs.reserve(inputLanes);
        let inventories = vec![0; outputLanes];

        let newMachine = Machine {
            id,
            cost,
            throughput,
            state,
            faultChance,
            faultMessage,

            processingBehavior: None,
            processingClock: 0,
            processingTickSpeed,
            processingTickHeld: false,
            
            inputBehavior: None,
            inputClock: 0,
            inputTickSpeed,
            inputTickHeld: false,
            inputIDs: inIDs,
            inputInventory: 0,
            inputInvCapacity,
            nextInput: 0,
            
            outputBehavior: None,
            outputClock: 0,
            outputTickSpeed,
            outputTickHeld: false,
            outputInventory: 0,
            outputInvCapacity,
            nextOutput: 0,

            outputLanes,
            beltCapacity,
            beltInventories: inventories,
            
            consumedCount: 0,
            producedCount: 0,
            stateChangeCount: 0,
        };

        return newMachine;
    }

    fn update(&mut self, deltaTime: u128, seed: i32, machines: &mut HashMap<usize, Machine>/*, input: &mut Belt, output: &mut Belt*/)
    {
        if self.state == OPCState::FAULTED
        {
            self.faulted();
            return;
        }
        
        self.processingClock += deltaTime;
        self.inputClock += deltaTime;
        self.outputClock += deltaTime;

        // Execute an input tick 
        if self.inputClock > self.inputTickSpeed || self.inputTickHeld
        {
            if self.inputBehavior.is_none()
            {
                println!("ID {}: Input behavior is not defined.", self.id);
                return;
            }
            let inputBehavior = self.inputBehavior.unwrap();
            // self.inputClock -= self.inputTickSpeed;
            self.inputClock = 0; // Set to 0 due to new tick holding system, may cause inaccuracy
            // Hold the tick on failure, so we do not wait needlessly
            self.inputTickHeld = !inputBehavior(self, machines);
        }
        
        // Execute a process tick 
        if self.processingClock > self.processingTickSpeed || self.processingTickHeld
        {
            if self.processingBehavior.is_none()
            {
                println!("ID {}: Processing behavior is not defined.", self.id);
                return;
            }
            let processingBehavior = self.processingBehavior.unwrap();
            // self.processingClock -= self.processingTickSpeed;
            self.processingClock = 0; // Set to 0 due to new tick holding system, may cause inaccuracy
            // Hold the tick on failure, so we do not wait needlessly
            self.processingTickHeld = !processingBehavior(self, seed);
        }

        // Execute a output tick 
        if self.outputClock > self.outputTickSpeed || self.outputTickHeld
        {
            if self.outputBehavior.is_none()
            {
                println!("ID {}: Output behavior is not defined.", self.id);
                return;
            }
            let outputBehavior = self.outputBehavior.unwrap();
            // self.outputClock -= self.outputTickSpeed;
            self.outputClock = 0; // Set to 0 due to new tick holding system, may cause inaccuracy
            // Hold the tick on failure, so we do not wait needlessly
            self.outputTickHeld = !outputBehavior(self);
        }
    }

    // Function for faulted state
    fn faulted(&mut self)
    {
        println!("ID {}: {}", self.id, self.faultMessage); //now prints the fault message from JSON
    }

    #[allow(unused_variables)]
    // Always has supply to input, like the start of a line
    fn spawnerInput(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        if self.inputInventory < self.inputInvCapacity
        {
            self.inputInventory += 1;
            return true;
        }

        return false;
    }

    // Inputs only if output is empty
    fn defaultInput(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        // check if space in input and output inventories
        if self.outputInventory > 0 || self.inputInventory >= self.inputInvCapacity
        {   
            return false; 
        }
      
        for _i in 0 as usize..self.inputIDs.len()
        {   
            let currentStructIDs = self.inputIDs[self.nextInput];
            // gets the machine of interest 
            let currentMachine = machines.get_mut(&currentStructIDs.machineID).expect("Value does not exist");
            // num of items on this belt
            let numOnBelt = currentMachine.beltInventories[currentStructIDs.laneID];

            // belt has something 
            if numOnBelt > 0
            {
                currentMachine.beltInventories[currentStructIDs.laneID] -= 1;
                self.inputInventory += 1;
                self.nextInput += 1;
                // stays the same, resets if out of bounds 
                self.nextInput = self.nextInput % self.inputIDs.len();
                return true;
                
            }

            self.nextInput += 1;
            self.nextInput = self.nextInput % self.inputIDs.len();
        }

        return false;
    }

    // Inputs even if there is something in the output
    fn flowInput(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        // check if space in input and output inventories
        if self.inputInventory >= self.inputInvCapacity
        {   
            return false; 
        }
      
        for _i in 0 as usize..self.inputIDs.len()
        {   
            let currentStructIDs = self.inputIDs[self.nextInput];
            // gets the machine of interest 
            let currentMachine = machines.get_mut(&currentStructIDs.machineID).expect("Value does not exist");
            // num of items on this belt
            let numOnBelt = currentMachine.beltInventories[currentStructIDs.laneID];

            // belt has something 
            if numOnBelt > 0
            {
                currentMachine.beltInventories[currentStructIDs.laneID] -= 1;
                self.inputInventory += 1;
                self.nextInput += 1;
                // stays the same, resets if out of bounds 
                self.nextInput = self.nextInput % self.inputIDs.len();
                return true;
                
            }

            self.nextInput += 1;
            self.nextInput = self.nextInput % self.inputIDs.len();
        }

        return false;
    }

    // Inputs one thing from every lane at once
    // fn multipleInput(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    // {
    //     // TODO
    //     return true;
    // }

    // Processess only if the output inventory is empty
    fn defaultProcessing(&mut self, seed: i32) -> bool
    {

        // check if enough input 
        if self.inputInventory < self.cost
        { 
            if self.state != OPCState::STARVED
            {
                self.state = OPCState::STARVED;
                self.stateChangeCount += 1;
                println!("ID {}: Starved.", self.id);
            }
            return false;
        }

        // check if room to output if processed
        if self.outputInventory != 0 || self.outputInvCapacity < self.throughput
        {
            if self.state != OPCState::BLOCKED
            {
                self.state = OPCState::BLOCKED;
                self.stateChangeCount += 1;
                println!("ID {}: Blocked.", self.id);
            }
            return false;
        }

        // process 
        self.inputInventory -= self.cost;
        self.consumedCount += self.cost;

        self.outputInventory += self.throughput;
        self.producedCount += self.throughput;

        if self.state != OPCState::PRODUCING
        {
            self.state = OPCState::PRODUCING;
            println!("ID {}: Switched back to producing state and produced.", self.id);
        }
        else {
            println!("ID {}: Produced.", self.id);
        }

        // TODO: fixable when auto recovery time added
        // Modulo seed by 1000, convert to float, convert to % (out of 1000), and compare to fail chance
        if (seed % 1000) as f32 / 1000.0 < self.faultChance
        {
            // Debug logging to show the seed when the machine faults
            println!("ID {}: {} {} {}", self.id, seed, seed % 1000, self.faultChance);
            self.state = OPCState::FAULTED;
            self.stateChangeCount += 1;
        }

        return true;
    }

    // Processes if there is enough room in output, even if not empty
    fn flowProcessing(&mut self, seed: i32) -> bool
    {
        // check if enough input 
        if self.inputInventory < self.cost
        { 
            if self.state != OPCState::STARVED
            {
                self.state = OPCState::STARVED;
                self.stateChangeCount += 1;
                println!("ID {}: Starved.", self.id);
            }
            return false;
        }

        // check if room to output if processed
        if self.outputInvCapacity - self.outputInventory < self.throughput
        {
            if self.state != OPCState::BLOCKED
            {
                self.state = OPCState::BLOCKED;
                self.stateChangeCount += 1;
                println!("ID {}: Blocked.", self.id);
            }
            return false;
        }
        
        // process 
        self.inputInventory -= self.cost;
        self.consumedCount += self.cost;

        self.outputInventory += self.throughput;
        self.producedCount += self.throughput;

        if self.state != OPCState::PRODUCING
        {
            self.state = OPCState::PRODUCING;
            println!("ID {}: Switched back to producing state and produced.", self.id);
        }
        else {
            println!("ID {}: Produced.", self.id);
        }

        // TODO: fixable when auto recovery time added
        // Modulo seed by 1000, convert to float, convert to % (out of 1000), and compare to fail chance
        if (seed % 1000) as f32 / 1000.0 < self.faultChance
        {
            // Debug logging to show the seed when the machine faults
            println!("ID {}: {} {} {}", self.id, seed, seed % 1000, self.faultChance);
            self.state = OPCState::FAULTED;
            self.stateChangeCount += 1;
        }

        return true;
    }

    // Outputs one thing onto one lane at a time
    fn defaultOutput(&mut self) -> bool
    {
        // iterate through outputLanes
        for _i in 0 as usize..self.outputLanes
        {
            // move 1 item from output inventory to nextOutput
            if self.outputInventory > 0 && self.beltInventories[self.nextOutput] < self.beltCapacity
            {
                self.beltInventories[self.nextOutput] += 1;
                self.outputInventory -= 1;
                self.nextOutput += 1;
                self.nextOutput = self.nextOutput % self.beltInventories.len();
                return true;
            }

            self.nextOutput += 1;
            self.nextOutput = self.nextOutput % self.beltInventories.len();
        } // end loop

        return false;
    }

    // Outputs one thing onto EVERY lane at once
    // fn multipleOutput(&mut self) -> bool
    // {
    //     // TODO
    //     return true;
    // }

    // Always has space to output, like the end of a line
    fn consumerOutput(&mut self) -> bool
    {
        if self.outputInventory > 0
        {
            self.outputInventory -= 1;
            return true;
        }

        return false;
    }
}

#[derive(Debug, Deserialize)]
struct JSONMachine {
    id: usize,
    cost: usize,
    throughput: usize,
    state: String,
    faultChance: f32,
    faultMessage: String,
    inputIDs: Vec<MachineLaneID>,
    inputBehavior: String,
    inputSpeed: u128, // ms
    inputCapacity: usize,
    processingBehavior: String,
    processingSpeed: u128,
    outputBehavior: String,
    outputSpeed: u128,
    outputCapacity: usize,
    outputLanes: usize,
    beltCapacity: usize,
}

#[derive(Debug, Deserialize)]
struct JSONFactory {
    name: String,
    description: String,
    simSpeed: f64,
    pollRate: u128,
    Runtime: u128,
    Machines: Vec<JSONMachine>,
}

#[derive(Debug, Deserialize)]
struct JSONData {
    factory: JSONFactory,
}

fn main() -> std::io::Result<()>
{
    // Debug backtrace info
    env::set_var("RUST_BACKTRACE", "1");

    // Tuple of HashMap<usize, Machine> and Vec<usize>
    let factoryData = factorySetup();
    // HashMap<usize, Machine>, associated machine ID with the relevant machine data
    let mut machines = factoryData.0;
    // Vec<usize>, tracks every valid ID, allows us to always get a valid hashmap entry
    let ids = factoryData.1;
    //Simulation speed
    let simSpeed: f64 = factoryData.2;
    // Server poll rate in milliseconds
    let pollRate = factoryData.3;
    let mut pollDeltaTime = 0; // time passed since last poll 

    let runtime = factoryData.4 * 1000;  // milliseconds needed to pass to stop
    let mut timePassed: u128 = 0; // milliseconds passed 

    // Set up the server and get a tuple containing the Server and a HashMap<usize, NodeId> of all nodes
    let serverData = serverSetup(machines.values().cloned().collect(), "MyLine");
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
    let mut iterTime:Duration = start.duration_since(UNIX_EPOCH).expect("Get epoch time in ms");
    let mut prevTime:Duration = iterTime;
    let mut deltaTime:u128;

    while timePassed < runtime
    {   
        // Find deltatime between loop iterations
        start = SystemTime::now();
        iterTime = start.duration_since(UNIX_EPOCH).expect("Get epoch time in ms");     
        deltaTime = iterTime.as_millis() - prevTime.as_millis();
        timePassed += deltaTime;
        deltaTime = ((iterTime.as_millis() as f64 * simSpeed) as u128) - (((prevTime.as_millis() as f64) * simSpeed) as u128);

        // rng is used to seed the update with any random integer, which is used for any rng dependent operationsu
        for id in ids.iter()
        {
            // TODO: Consider multiple passes over machines with some logic, in the case of extremely fast machines or extremely high sim speed
            let mut machineCopy = machines.get(&id).unwrap().clone();
            machineCopy.update(deltaTime, rng.gen_range(0..=std::i32::MAX), &mut machines);
            machines.insert(*id, machineCopy);
        }

        // Check if the server should poll for updates
        pollDeltaTime += deltaTime;
        if pollDeltaTime >= pollRate
        {
            pollDeltaTime -= pollRate;
            let mut addressSpace = addressSpace.write();
            serverPoll(&mut addressSpace, &machines, &nodeIDs, &ids);
        }

        // Log system time at the start of this iteration, for use in next iteration
        prevTime = iterTime;
    }
    let file = File::create("log.txt")?;
    for id in ids.iter()
    {
        let machine = machines.get(id).unwrap();
        let efficiencyCount = machine.producedCount as f64 / (machine.throughput as f64 * (runtime as f64 / machine.processingTickSpeed as f64));

        writeln!(&file, "Machine ID: {}", machines.get(id).expect("Machine ceased to exist").id)?;
        writeln!(&file, "Machine Input: {}", machines.get(id).expect("Machine ceased to exist").consumedCount)?;
        writeln!(&file, "Machine Output: {}", machines.get(id).expect("Machine ceased to exist").producedCount)?;
        writeln!(&file, "State Changes: {}", machines.get(id).expect("Machine ceased to exist").stateChangeCount)?;
        writeln!(&file, "Efficiency: {}", efficiencyCount)?;
        writeln!(&file, "")?;

    }
    Ok(())
}

fn read_json_file(file_path: &str) -> String {
    let mut file_content = String::new();
    let mut file = File::open(file_path).expect("Failed to open file");
    file.read_to_string(&mut file_content).expect("Failed to read file content");
    file_content
}

fn factorySetup() -> (HashMap<usize, Machine>, Vec<usize>, f64, u128, u128)
{
    let file_path = "factory.json";
    let json_data = read_json_file(file_path);
    let data: JSONData = serde_json::from_str(&json_data).expect("Failed to parse JSON");

    println!("Factory Name: {}", data.factory.name);
    println!("Description: {}", data.factory.description);
    println!("simSpeed: {} ", data.factory.simSpeed);
    println!("pollRate: {} milliseconds", data.factory.pollRate);
    println!("Runtime: {} seconds", data.factory.Runtime);
    println!("");

    //Setting data to variables to be passed into the return
    let factorySpeed = data.factory.simSpeed; 
    let factoryPollRate = data.factory.pollRate;
    let factoryRuntime = data.factory.Runtime;

    let mut machines = HashMap::<usize, Machine>::new();
    let mut ids = Vec::<usize>::new(); // Track all IDs, this makes iterating over the hashmap easier in the future

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
        
        let mut newMachine = Machine::new(
            machine.id,
            machine.cost,
            machine.throughput,
            state,
            machine.faultChance,
            machine.faultMessage,
            machine.processingSpeed,
            machine.inputSpeed,
            machine.inputIDs.len(),
            machine.inputCapacity,
            machine.outputSpeed,
            machine.outputCapacity,
            machine.outputLanes,
            machine.beltCapacity
        );
        newMachine.inputIDs = machine.inputIDs;

        let mut inputBehavior: fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool = Machine::defaultInput;
        let mut processingBehavior: fn(&mut Machine, i32) -> bool = Machine::defaultProcessing;
        let mut outputBehavior: fn(&mut Machine) -> bool = Machine::defaultOutput;
        match machine.inputBehavior.to_lowercase().as_str()
        {
            "spawner" => inputBehavior = Machine::spawnerInput,
            "default" => inputBehavior = Machine::defaultInput,
            "flow" => inputBehavior = Machine::flowInput,
            _ => (),
        }
        match machine.processingBehavior.to_lowercase().as_str()
        {
            "default" => processingBehavior = Machine::defaultProcessing,
            "flow" => processingBehavior = Machine::flowProcessing,
            _ => (),
        }
        match machine.outputBehavior.to_lowercase().as_str()
        {
            "consumer" => outputBehavior = Machine::consumerOutput,
            "default" => outputBehavior = Machine::defaultOutput,
            _ => (),
        }

        newMachine.inputBehavior = Some(inputBehavior);
        newMachine.processingBehavior = Some(processingBehavior);
        newMachine.outputBehavior = Some(outputBehavior);

        ids.push(machine.id);
        machines.insert(machine.id, newMachine);
    }

    return (machines, ids, factorySpeed, factoryPollRate, factoryRuntime);
}

// Returns a tuple containing the new Server, as well as a HashMap of machine IDs to OPC NodeIDs
fn serverSetup(machines: Vec<Machine>, lineName: &str) -> (Server, HashMap<String, NodeId>)
{
    // let server = Server::new(ServerConfig::load(&PathBuf::from("./server.conf")).unwrap());
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
fn serverPoll(addressSpace: &mut AddressSpace, machines: &HashMap<usize, Machine>, nodeIDs: &HashMap<String, NodeId>, ids: &Vec<usize>)
{
    let now = DateTime::now();
    for id in ids.iter()
    {
        let machine = machines.get(&id).expect("Machine ceased to exist.");
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

fn initServer() -> Server
{
    let ipAddress = local_ip().expect("IP could not be found.");
    let hostName = hostname().expect("Hostname could not be found.");
    let discoveryURL = format!("opc.tcp://{ipAddress}:4855/");
    println!("Discovery URL: {}", discoveryURL);

    let server = ServerBuilder::new()
        .application_name("OPC UA Simulation Server")
        .application_uri("urn:OPC UA Simulation Server")
        .create_sample_keypair(true)
        .certificate_path(&PathBuf::from("own/cert.der"))
        .private_key_path(&PathBuf::from("private/private.pem"))
        .pki_dir("./pki-server")
        .discovery_server_url(None)
        .host_and_port(ipAddress.to_string(), 4855)
        .discovery_urls(vec![format!("/"), format!("opc.tcp://{hostName}:4855/")])
        .endpoints(
            [
                ("none", "/", SecurityPolicy::None, MessageSecurityMode::None, &["ANONYMOUS"]),
                ("basic128rsa15_sign", "/", SecurityPolicy::Basic128Rsa15, MessageSecurityMode::Sign, &["ANONYMOUS"]),
                ("basic128rsa15_sign_encrypt", "/", SecurityPolicy::Basic128Rsa15, MessageSecurityMode::SignAndEncrypt, &["ANONYMOUS"]),
                ("basic256_sign", "/", SecurityPolicy::Basic256, MessageSecurityMode::Sign, &["ANONYMOUS"]),
                ("basic256_sign_encrypt", "/", SecurityPolicy::Basic256, MessageSecurityMode::SignAndEncrypt, &["ANONYMOUS"]),
                ("basic256sha256_sign", "/", SecurityPolicy::Basic256Sha256, MessageSecurityMode::Sign, &["ANONYMOUS"]),
                ("basic256sha256_sign_encrypt", "/", SecurityPolicy::Basic256Sha256, MessageSecurityMode::SignAndEncrypt, &["ANONYMOUS"]),
            ].iter().map(|v| {
                (v.0.to_string(), ServerEndpoint::from((v.1, v.2, v.3, &v.4[..])))
            }).collect())
        .server().unwrap();
    return server;
}
