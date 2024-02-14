#![allow(non_snake_case)]

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
use opcua::server::prelude::*;


#[derive(Clone, PartialEq, Eq)]
enum OPCState
{
    PRODUCING,
    FAULTED,
    BLOCKED,
    STARVED,
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
    processClock: u128, // deltaTime is in milliseconds
    processTickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    failChance: f32,
    cost: usize, // Cost to produce
    throughput: usize, // How much gets produced
    state: OPCState,
    faultMessage: String, //string for fault messages 
    inputLanes: usize, // Number of input lanes
    inputIDs: Vec<MachineLaneID>, // Vector of machine/lane IDs for input, used as indices
    inBehavior: Option<fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool>, // Function pointer that can also be None, used to define behavior
    outputLanes: usize,
    outBehavior: Option<fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool>,
    capacity: usize, // Capacity of EACH beltInventories
    beltInventories: Vec<usize>, // Vector of inventories, one per output lane

    producedCount: usize,
    consumedCount: usize,
    stateChangeCount: usize,
}
impl Machine
{
    fn new(id: usize, processTickSpeed: u128, failChance: f32, cost: usize, throughput: usize, state: OPCState, faultMessage: String, inputLanes: usize, outputLanes: usize, capacity: usize) -> Self
    {
        let mut inIDs = Vec::<MachineLaneID>::new();
        inIDs.reserve(inputLanes);
        let inventories = vec![0; outputLanes];

        let newMachine = Machine {
            id,
            processClock: 0,
            processTickSpeed,
            failChance,
            cost,
            throughput,
            state,
            faultMessage,
            inputLanes,
            inputIDs: inIDs,
            inBehavior: None,
            outputLanes,
            outBehavior: None,
            capacity,
            beltInventories: inventories,
            consumedCount: 0,
            producedCount: 0,
            stateChangeCount: 0,
        };
        return newMachine;
    }

    fn set_behavior(&mut self, inBehavior: fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool, outBehvaior: fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool)
    {
        self.inBehavior = Some(inBehavior);
        self.outBehavior = Some(outBehvaior);
    }

    fn update(&mut self, deltaTime: u128, seed: i32, machines: &mut HashMap<usize, Machine>/*, input: &mut Belt, output: &mut Belt*/)
    {
        self.processClock += deltaTime;
        
        // If it is not time to execute a tick, return
        if self.processClock < self.processTickSpeed
        {
            return;
        }

        // Execute a tick
        self.processClock -= self.processTickSpeed;
        match self.state
        {
            OPCState::PRODUCING=>self.producing(seed, machines),
            OPCState::FAULTED=>self.faulted(),
            OPCState::BLOCKED=>self.blocked(machines),
            OPCState::STARVED=>self.starved(machines),
        }
    }

    // Function for producing state
    fn producing(&mut self, seed: i32, machines: &mut HashMap<usize, Machine>/*, input: &mut Belt, output: &mut Belt*/)
    {
        //println!("Producing.");
        if self.inBehavior.is_none() || self.outBehavior.is_none()
        {
            println!("ID {}: One or more behaviors' function pointer is None", self.id);
            return;
        }

        // TODO: make this less awful, try to make it so it doesnt draw from input before it knows if its blocked or not
        let mut invBackups = Vec::<usize>::new();
        for i in 0..self.inputIDs.len()
        {
            invBackups.push(machines.get(&self.inputIDs[i].machineID).unwrap().beltInventories[self.inputIDs[i].laneID]);
        }

        // Expect gets the contents of a "Some" Option, and throws the given error message if it is None
        let inBehavior = self.inBehavior.expect("This shouldn't be possible.");
        if inBehavior(self, machines)
        {
            let outBehavior = self.outBehavior.expect("This shouldn't be possible.");
            if outBehavior(self, machines)
            {
                println!("ID {}: Pushed", self.id);
                self.producedCount += self.throughput;
                self.consumedCount += self.cost;
            }
            else
            {
                // TODO: make this less awful, try to make it so it doesnt draw from input before it knows if its blocked or not
                for i in 0..self.inputIDs.len()
                {
                    machines.get_mut(&self.inputIDs[i].machineID).unwrap().beltInventories[self.inputIDs[i].laneID] = invBackups[i];
                }

                // enough input, but can't output
                self.state = OPCState::BLOCKED;
                self.stateChangeCount += 1;
                println!("ID {}: Blocked, no room to output", self.id);
            }
        }
        else
        {
            // not enough input
            self.state = OPCState::STARVED; 
            self.stateChangeCount += 1;
            println!("ID {}: Starved, not enough supply", self.id);
        }

        // Modulo seed by 1000, convert to float, convert to % (out of 1000), and compare to fail chance
        if (seed % 1000) as f32 / 1000.0 < self.failChance
        {
            // Debug logging to show the seed when the machine faults
            println!("ID {}: {} {} {}", self.id, seed, seed % 1000, self.failChance);
            self.state = OPCState::FAULTED;
            self.stateChangeCount += 1;
        }
    }

    // Function for faulted state
    fn faulted(&mut self)
    {
        println!("ID {}: {}", self.id, self.faultMessage); //now prints the fault message from JSON
    }


    fn blocked(&mut self, machines: &mut HashMap<usize, Machine>) 
    {
        //Error Check if returns is_non, error and exit.
        if self.inBehavior.is_none() || self.outBehavior.is_none()
        {
            println!("ID {}: One or more behaviors' function pointer is None", self.id);
            return;
        }

        // TODO: make this less awful, try to make it so it doesnt draw from input before it knows if its blocked or not
        let mut invBackups = Vec::<usize>::new();
        for i in 0..self.inputIDs.len()
        {
            invBackups.push(machines.get(&self.inputIDs[i].machineID).unwrap().beltInventories[self.inputIDs[i].laneID]);
        }

        // Expect gets the contents of a "Some" Option, and throws the given error message if it is None
        let inBehavior = self.inBehavior.expect("This shouldn't be possible.");
        if inBehavior(self, machines)
        {
            let outBehavior = self.outBehavior.expect("This shouldn't be possible.");
            if outBehavior(self, machines)
            {
                println!("ID {}: Pushed, Switched to Producing", self.id);
                self.state = OPCState::PRODUCING;
                self.stateChangeCount += 1;
                self.producedCount += self.throughput;
                self.consumedCount += self.cost;
            }
            else
            {
                // TODO: make this less awful, try to make it so it doesnt draw from input before it knows if its blocked or not
                for i in 0..self.inputIDs.len()
                {
                    machines.get_mut(&self.inputIDs[i].machineID).unwrap().beltInventories[self.inputIDs[i].laneID] = invBackups[i];
                }

                //still blocked stay that way
                println!("ID {}: Blocked.", self.id);
            }
        }
        else
        {
            //needs dual states implimented for when both blocked and starved
            // track state change too
        }
    }

    fn starved(&mut self, machines: &mut HashMap<usize, Machine>) 
    {
        //Error Check if returns is_non, error and exit.
        if self.inBehavior.is_none() || self.outBehavior.is_none()
        {
            println!("ID {}: One or more behaviors' function pointer is None", self.id);
            return;
        }

        // TODO: make this less awful, try to make it so it doesnt draw from input before it knows if its blocked or not
        let mut invBackups = Vec::<usize>::new();
        for i in 0..self.inputIDs.len()
        {
            invBackups.push(machines.get(&self.inputIDs[i].machineID).unwrap().beltInventories[self.inputIDs[i].laneID]);
        }

        // Expect gets the contents of a "Some" Option, and throws the given error message if it is None
        let inBehavior = self.inBehavior.expect("This shouldn't be possible.");
        if inBehavior(self, machines)
        {
            let outBehavior = self.outBehavior.expect("This shouldn't be possible.");
            if outBehavior(self, machines)
            {
                println!("ID {}: Pushed, Switched to Producing", self.id);
                self.state = OPCState::PRODUCING;
                self.stateChangeCount += 1;
                self.producedCount += self.throughput;
                self.consumedCount += self.cost;
            }
            else
            {
                // TODO: make this less awful, try to make it so it doesnt draw from input before it knows if its blocked or not
                for i in 0..self.inputIDs.len()
                {
                    machines.get_mut(&self.inputIDs[i].machineID).unwrap().beltInventories[self.inputIDs[i].laneID] = invBackups[i];
                }
                
                //If output blocked change to blocked state
                println!("ID {}: Output is Blocked, changing state.", self.id);
                self.state = OPCState::BLOCKED;
                self.stateChangeCount += 1;
            }
        }
        else
        {
            println!("ID {}: Starved.", self.id); 
        }
    }

    // Algorithm for evenly pulling from multiple input lanes, favoring lower IDs/indices for imbalances
    // This algorithm can assume there is room for the input, because it will only be called if there IS room
    fn multilane_pull(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        // Calculate the ideal amount of things to take from each lane
        let mut inPerLane = vec![0; self.inputLanes];
        for i in 0..self.inputLanes
        {
            inPerLane[i] = self.cost / self.inputLanes;
        }
        inPerLane[0] += self.cost % self.inputLanes;

        // Find what lanes possess this ideal amount to supply, and what lanes do not
        let mut needed = 0; // Track how much excess is needed from lanes that have extra
        for i in 0..self.inputLanes
        {
            let currentID = self.inputIDs[i];
            if machines.get(&currentID.machineID).unwrap().beltInventories[currentID.laneID] < inPerLane[i]
            {
                let floating = inPerLane[i] - machines.get(&currentID.machineID).unwrap().beltInventories[currentID.laneID];
                inPerLane[i] -= floating;
                needed += floating;
            }
        }

        // Try to draw the needed amount from lanes that have an amount of excess available
        for i in 0..self.inputLanes
        {
            let currentID = self.inputIDs[i];
            let mut available = machines.get(&currentID.machineID).unwrap().beltInventories[currentID.laneID] - inPerLane[i]; 
            if needed != 0 && available > 0
            {
                if available > needed { available = needed; }
                inPerLane[i] += available;
                needed -= available;
            }
        }

        // If all the need could be fulfilled, subtract the input and signal that it has been taken
        if needed == 0
        {
            for i in 0..self.inputLanes
            {
                let currentID = self.inputIDs[i];
                // println!("{} {}", machines.get_mut(&currentID.machineID).unwrap().beltInventories[currentID.laneID], inPerLane[i]);
                machines.get_mut(&currentID.machineID).expect("Machine HashMap error").beltInventories[currentID.laneID] -= inPerLane[i];
            }
            return true;
        }

        // This only happens if demand could not be met
        return false;
    }

    #[allow(unused_variables)]
    // Always returns true, to simulate having infinite supply
    fn spawner_input(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        return true;
    }

    #[allow(unused_variables)]
    // Algorithm for evenly pushing output onto multiple lanes, favoring lower IDs/indices for imbalances
    fn multilane_push(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        // Calculating the ideal amount of things to put on each output lane
        let mut outPerLane = vec![0; self.outputLanes];
        for i in 0..self.outputLanes
        {
            outPerLane[i] = self.throughput / self.outputLanes;
        }
        outPerLane[0] += self.throughput % self.outputLanes;

        // Find what will fit without shifting any around
        let mut remaining = 0; // Remaining product that is not yet on a lane
        for i in 0..self.outputLanes
        {
            let sum = self.beltInventories[i] + outPerLane[i];
            if sum > self.capacity
            {
                // Put the overflow in the remaining variable, keep the amount that will fit
                let floating = sum - self.capacity;
                outPerLane[i] -= floating;
                remaining += floating;
            }
        }

        // Put the remaining products in the nearest available space, on the next ID/index up
        for i in 0..self.outputLanes
        {
            let sum = self.beltInventories[i] + outPerLane[i];
            if remaining != 0 && sum < self.capacity
            {
                let mut available = self.capacity - sum;
                if available > remaining { available = remaining; }
                outPerLane[i] += available;
                remaining -= available;
            }
        }

        // If all the output could fit, take input and produce output
        if remaining == 0
        {
            for i in 0..self.outputLanes
            {
                self.beltInventories[i] += outPerLane[i];
            }
            return true;
        }

        // This only happens if the output could not fit
        return false;
    }

    // Uses multilane_push, but just zeroes out every beltInventories afterwards, for infinite space 
    fn consumer_output(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        if !self.multilane_push(machines) { return false; }

        for i in 0..self.beltInventories.len()
        {
            self.beltInventories[i] = 0;
        }
        
        return true;
    }
}

#[derive(Debug, Deserialize)]
struct JSONMachine {
    id: usize,
    tickSpeed: u128,
    failChance: f32,
    cost: usize,
    throughput: usize,
    state: String,
    faultMessage: String,
    inputIDs: Vec<MachineLaneID>,
    inBehavior: String,
    outputLanes: usize,
    outBehavior: String,
    capacity: usize,
}

#[derive(Debug, Deserialize)]
struct JSONFactory {
    name: String,
    description: String,
    Runtime: u32,
    Machines: Vec<JSONMachine>,
}

#[derive(Debug, Deserialize)]
struct JSONData {
    factory: JSONFactory,
}

fn read_json_file(file_path: &str) -> String {
    let mut file_content = String::new();
    let mut file = File::open(file_path).expect("Failed to open file");
    file.read_to_string(&mut file_content).expect("Failed to read file content");
    file_content
}


fn main() -> std::io::Result<()>
{
    // Debug backtrace info
    env::set_var("RUST_BACKTRACE", "1");

    let file_path = "factory.json";
    let json_data = read_json_file(file_path);
    let data: JSONData = serde_json::from_str(&json_data).expect("Failed to parse JSON");

    println!("Factory Name: {}", data.factory.name);
    println!("Description: {}", data.factory.description);
    println!("Runtime: {} seconds", data.factory.Runtime);
    println!("");

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
            machine.tickSpeed,
            machine.failChance,
            machine.cost,
            machine.throughput,
            state,
            machine.faultMessage,
            machine.inputIDs.len(),
            machine.outputLanes,
            machine.capacity
        );
        newMachine.inputIDs = machine.inputIDs;

        let mut inBehavior: fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool = Machine::multilane_pull;
        let mut outBehavior: fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool = Machine::multilane_push;
        match machine.inBehavior.to_lowercase().as_str()
        {
            "spawner" => inBehavior = Machine::spawner_input,
            "default" => inBehavior = Machine::multilane_pull,
            _ => (),
        }
        match machine.outBehavior.to_lowercase().as_str()
        {
            "consumer" => outBehavior = Machine::consumer_output,
            "default" => outBehavior = Machine::multilane_push,
            _ => (),
        }

        newMachine.set_behavior(inBehavior, outBehavior);

        ids.push(machine.id);
        machines.insert(machine.id, newMachine);

        // println!("Machine ID: {}", data.factory.Machines[i].id);
        // println!("Tick-Speed: {}", data.factory.Machines[i].tickSpeed);
        // println!("failChance: {}", data.factory.Machines[i].failChance);
        // println!("Cost: {}", data.factory.Machines[i].cost);
        // println!("Throughput: {}", data.factory.Machines[i].throughput);
        // println!("State: {}", data.factory.Machines[i].state);
        // println!("Input Lanes: {}", data.factory.Machines[i].inputLanes);
        // println!("Input ID: {:?}, {:?}", data.factory.Machines[i].inputIDs[0].machineID, data.factory.Machines[i].inputIDs[0].laneID);
        // println!("Input Behavior: {}", data.factory.Machines[i].inBehavior);
        // println!("Output Lanes: {}", data.factory.Machines[i].outputLanes);
        // println!("Output Behavior: {}", data.factory.Machines[i].outBehavior);
        // println!("Capacity: {}", data.factory.Machines[i].capacity);
        // println!("");

    }
    
    // This is VERY work in progress, this is just a test case
    // machines.insert(0, Machine::new(0, 500, 0.0, 1, 3, OPCState::PRODUCING, 1, 3, 6));
    // ids.push(0);
    // let mut curMachine: &mut Machine = machines.get_mut(&0).unwrap();
    // curMachine.set_behavior(Machine::spawner_input, Machine::multilane_push);

    // machines.insert(14, Machine::new(14, 1000, 0.0, 6, 2, OPCState::PRODUCING, 3, 2, 4));
    // ids.push(14);
    // curMachine = machines.get_mut(&14).unwrap();
    // curMachine.connect(0, 0);
    // curMachine.connect(0, 1);
    // curMachine.connect(0, 2);
    // curMachine.set_behavior(Machine::multilane_pull, Machine::multilane_push);

    // machines.insert(2, Machine::new(2, 1000, 0.0, 1, 1, OPCState::PRODUCING, 1, 1, 2));
    // ids.push(2);
    // curMachine = machines.get_mut(&2).unwrap();
    // curMachine.connect(14, 0);
    // curMachine.set_behavior(Machine::multilane_pull, Machine::consumer_output);

    // machines.insert(3, Machine::new(3, 1000, 0.0, 1, 1, OPCState::PRODUCING, 1, 1, 2));
    // ids.push(3);
    // curMachine = machines.get_mut(&3).unwrap();
    // curMachine.connect(14, 1);
    // curMachine.set_behavior(Machine::multilane_pull, Machine::consumer_output);

    let runtime = 10 * 1000;  // milliseconds needed to pass to stop
    let mut timePassed: u128 = 0; // milliseconds passed 
    
    //Simulation speed
    let simSpeed: f64 = 1.0;
    
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

        // Log system time at the start of this iteration, for use in next iteration
        prevTime = iterTime;
    }
    let file = File::create("log.txt")?;
    for id in ids.iter()
    {
        let efficiencyCount = machines.get(id).expect("idk").producedCount / machines.get(id).expect("idk").capacity * runtime as usize;
        writeln!(&file, "Machine ID: {}", machines.get(id).expect("Machine ceased to exist").id)?;
        writeln!(&file, "Machine Input: {}", machines.get(id).expect("Machine ceased to exist").consumedCount)?;
        writeln!(&file, "Machine Output: {}", machines.get(id).expect("Machine ceased to exist").producedCount)?;
        writeln!(&file, "State Changes: {}", machines.get(id).expect("Machine ceased to exist").stateChangeCount)?;
        writeln!(&file, "Efficiency: {}", efficiencyCount)?;
        writeln!(&file, "")?;

    }
    Ok(())
}

fn serverSetup()
{
    // setup here
}
