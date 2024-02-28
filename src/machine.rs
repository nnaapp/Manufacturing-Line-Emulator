use std::fmt;
use std::collections::HashMap;
use std::cell::RefCell;
use std::cell::RefMut;

extern crate serde_json;
extern crate serde;
use self::serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OPCState
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

pub struct MachineLine
{
    pub machines: HashMap<usize, RefCell<Machine>>,
    pub ids: Vec<usize>,
}
impl MachineLine
{
    pub fn update(&mut self, deltaTime: u128, seed: i32)
    {
        for id in self.ids.clone().iter()
        {
            {
                let mut machine = self.machines.get(id).expect("Machine ceased to exist.").borrow_mut();
                machine.updateBelts(deltaTime);
            }

            {
                let mut machine = self.machines.get(id).expect("Machine ceased to exist.").borrow_mut();
                if machine.state != OPCState::FAULTED
                {
                    // Execute input
                    // Input needs to manage: 
                    //     inputInProgress
                    //     inputWaiting
                    //     inputClock
                    if machine.inputBehavior.is_none()
                    {
                        println!("ID {}: Input behavior is not defined.", machine.id);
                        machine.faultMessage = format!("Simulation Error: input behavior not defined.");
                        return;
                    }
                    let inputBehavior = machine.inputBehavior.unwrap();
                    inputBehavior(self, &mut machine, deltaTime);
                }
            }

            {
                let mut machine = self.machines.get(id).expect("Machine ceased to exist.").borrow_mut();
                if machine.state != OPCState::FAULTED
                {
                    // Execute processing 
                    // Processing needs to manage:
                    //     processingInProgress
                    //     processingClock
                    if machine.processingBehavior.is_none()
                    {
                        println!("ID {}: Processing behavior is not defined.", machine.id);
                        machine.faultMessage = format!("Simulation Error: processing behavior not defined.");
                        return;
                    }
                    let processingBehavior = machine.processingBehavior.unwrap();
                    processingBehavior(self, &mut machine, deltaTime, seed);
                }
            }

            {
                let mut machine = self.machines.get(id).expect("Machine ceased to exist.").borrow_mut();

                if machine.state == OPCState::FAULTED
                {
                    machine.faulted();
                }
            }

            {
                let mut machine = self.machines.get(id).expect("Machine ceased to exist.").borrow_mut();
                // Execute output
                // Output needs to manage:
                //     outputInProgress
                //     outputWaiting
                //     outputClock
                if machine.outputBehavior.is_none()
                {
                    println!("ID {}: Output behavior is not defined.", machine.id);
                    machine.faultMessage = format!("Simulation Error: output behavior not defined.");
                    return;
                }
                let outputBehavior = machine.outputBehavior.unwrap();
                outputBehavior(self, &mut machine, deltaTime);
            }
        }
    }

    fn findInputSingle(&self, machine: &mut RefMut<Machine>) -> bool
    {
        if machine.inputInventory >= machine.inputInvCapacity
        {   
            return false; 
        }

        for _i in 0 as usize..machine.inputIDs.len()
        {   
            let currentStructIDs = machine.inputIDs[machine.nextInput];
            // gets the machine of interest 
            let mut currentMachine = self.machines.get(&currentStructIDs.machineID).expect("Value does not exist").borrow_mut();

            // belt has something 
            if currentMachine.checkBeltSupply(currentStructIDs.laneID) == true { return true; }

            machine.nextInput += 1;
            machine.nextInput = machine.nextInput % machine.inputIDs.len();
        }

        return false;
    }

    fn findOutputSingle(&self, machine: &mut RefMut<Machine>) -> bool
    {
        if machine.outputInventory <= 0
        {
            return false;
        }
        
        for _i in 0 as usize..machine.outputLanes
        {
            if machine.beltInventories[machine.nextOutput][0].is_none() { return true; }

            machine.nextOutput += 1;
            machine.nextOutput = machine.nextOutput % machine.beltInventories.len();
        }
        
        return false;
    }

    #[allow(unused_variables)]
    // Always has supply to input, like the start of a line
    pub fn spawnerInput(&self, machine: &mut RefMut<Machine>, deltaTime: u128) -> bool
    {
        if !machine.inputInProgress && (machine.inputInventory < machine.inputInvCapacity)
        {
            // Set the input to be in progress
            machine.inputWaiting = true;
            machine.inputInProgress = true;
            machine.inputClock = 0;
        }

        if !machine.inputInProgress 
        { 
            machine.inputWaiting = false;
            return false; 
        }

        if machine.inputClock < machine.inputTickSpeed
        {
            machine.inputClock += deltaTime;
            return false;
        }

        machine.inputInventory += 1;
        machine.inputInProgress = false;
        return true;        
    }
    
    // Inputs only ONE thing if output is empty,
    // ASSUMES that findOutputSingle has been called
    pub fn singleInput(&self, machine: &mut RefMut<Machine>, deltaTime: u128) -> bool
    {
        if !machine.inputInProgress && machine.outputInventory <= 0 
        {
            if !self.findInputSingle(machine) { return false; }
            // gets the machine of interest 
            let currentMachineLaneID = machine.inputIDs[machine.nextInput];
            let mut currentMachine = self.machines.get(&currentMachineLaneID.machineID).expect("Value does not exist").borrow_mut();
            let currentLane = currentMachineLaneID.laneID;
            let beltCapacity = currentMachine.beltCapacity;
            // Take 1 item off it (reserve so nothing else can take it, essentially)
            currentMachine.beltInventories[currentLane][beltCapacity - 1] = None;
            // Increment nextInput for balanced taking of items
            machine.nextInput += 1;
            machine.nextInput = machine.nextInput % machine.inputIDs.len();

            // Set the input to be in progress
            machine.inputWaiting = true;
            machine.inputInProgress = true;
            machine.inputClock = 0;
        }

        if !machine.inputInProgress 
        { 
            machine.inputWaiting = false;
            return false; 
        }

        if machine.inputClock < machine.inputTickSpeed
        {
            machine.inputClock += deltaTime;
            return false;
        }

        machine.inputInventory += 1;
        machine.inputInProgress = false;
        return true;
    }

    // Processess only if the output inventory is empty
    pub fn defaultProcessing(&self, machine: &mut RefMut<Machine>, deltaTime: u128, seed: i32) -> bool
    {
        if !machine.processingInProgress && !machine.inputWaiting && !machine.outputWaiting
        {
            // check if enough input 
            if machine.inputInventory < machine.cost
            { 
                if machine.state != OPCState::STARVED
                {
                    machine.state = OPCState::STARVED;
                    machine.stateChangeCount += 1;
                    println!("ID {}: Starved.", machine.id);
                }
                return false;
            }

            // check if room to output if processed
            if machine.outputInventory != 0 || machine.outputInvCapacity < machine.throughput
            {
                if machine.state != OPCState::BLOCKED
                {
                    machine.state = OPCState::BLOCKED;
                    machine.stateChangeCount += 1;
                    println!("ID {}: Blocked.", machine.id);
                }
                return false;
            }
        }

        if !machine.processingInProgress
            && machine.inputInventory >= machine.cost
            && machine.outputInventory == 0 
            && machine.outputInvCapacity >= machine.throughput
        {
            machine.processingInProgress = true;
            machine.processingClock = 0;
        }

        if !machine.processingInProgress { return false; }

        if machine.processingClock < machine.processingTickSpeed
        {
            machine.processingClock += deltaTime;
            return false;
        }

        if machine.checkIfShouldFault(seed) { return false; }
        
        // process 
        machine.inputInventory -= machine.cost;
        machine.consumedCount += machine.cost;

        machine.outputInventory += machine.throughput;
        machine.producedCount += machine.throughput;

        if machine.state != OPCState::PRODUCING
        {
            machine.state = OPCState::PRODUCING;
            println!("ID {}: Switched back to producing state and produced.", machine.id);
        }
        else {
            println!("ID {}: Produced.", machine.id);
        }

        machine.processingInProgress = false;
        return true;
    }

    // Outputs one thing onto one lane at a time
    pub fn singleOutput(&self, machine: &mut RefMut<Machine>, deltaTime: u128) -> bool
    {
        if !machine.outputInProgress && machine.outputInventory > 0
        {
            if !self.findOutputSingle(machine) { return false; }

            // Debating this one, unsure if this should be pre or post clock
            // machine.outputInventory -= 1;
            machine.outputWaiting = true;
            machine.outputInProgress = true;
            machine.outputClock = 0;
        }

        if !machine.outputInProgress 
        { 
            machine.outputWaiting = false;
            return false; 
        }

        if machine.outputClock < machine.outputTickSpeed
        {
            machine.outputClock += deltaTime;
            return false;
        }

        machine.outputInventory -= 1;
        let nextOutput = machine.nextOutput;
        machine.beltInventories[nextOutput][0] = Some(BeltItem { moveClock: 0, tickSpeed: machine.beltTickSpeed, isMoving: false });

        machine.nextOutput += 1;
        machine.nextOutput = machine.nextOutput % machine.beltInventories.len();

        machine.outputInProgress = false;
        return true;
    }

    // Always has space to output, like the end of a line
    pub fn consumerOutput(&self, machine: &mut RefMut<Machine>, deltaTime: u128) -> bool
    {
        if !machine.outputInProgress && machine.outputInventory > 0
        {
            machine.outputWaiting = true;
            machine.outputInProgress = true;
            machine.outputClock = 0;
        }

        if !machine.outputInProgress 
        { 
            machine.outputWaiting = false;
            return false; 
        }

        if machine.outputClock < machine.outputTickSpeed
        {
            machine.outputClock += deltaTime;
            return false;
        }

        machine.outputInventory -= 1;
        machine.outputInProgress = false;
        return true;
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct MachineLaneID
{
    pub machineID: usize,
    pub laneID: usize,
}

#[derive(Clone)]
pub struct BeltItem
{
    pub moveClock: u128, // clock for current movement
    pub tickSpeed: u128, // time it takes to perform a movement
    pub isMoving: bool,
}

#[derive(Clone)]
pub struct Machine
{
    pub id: usize,
    pub cost: usize, // Cost to produce
    pub throughput: usize, // How much gets produced
    pub state: OPCState,
    pub faultChance: f32,
    pub faultMessage: String, //string for fault messages 

    pub processingBehavior: Option<fn(&MachineLine, &mut RefMut<Machine>, u128, i32) -> bool>, 
    pub processingClock: u128, // deltaTime is in milliseconds
    pub processingTickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    pub processingInProgress: bool,

    pub inputBehavior: Option<fn(&MachineLine, &mut RefMut<Machine>, u128) -> bool>, // Function pointer that can also be None, used to define behavior
    pub inputClock: u128,
    pub inputTickSpeed: u128, // tick for pulling input into inputInventory
    pub inputInProgress: bool,
    pub inputWaiting: bool, // is there room for input, and input to be taken?
    pub inputIDs: Vec<MachineLaneID>, // Vector of machine/lane IDs for input, used as indices
    pub inputInventory: usize, // storage place in machine before process 
    pub inputInvCapacity: usize, 
    pub nextInput: usize, // the input lane to start checking from 

    pub outputBehavior: Option<fn(&MachineLine, &mut RefMut<Machine>, u128) -> bool>,
    pub outputClock: u128, 
    pub outputTickSpeed: u128, // tick for outputting 
    pub outputInProgress: bool,
    pub outputWaiting: bool, // is there output in the machine, and room to spit it out?
    pub outputInventory: usize, // represents num of items in it 
    pub outputInvCapacity: usize,
    pub nextOutput: usize, // the output lane to start checkng from

    pub outputLanes: usize, // number out output lanes
    pub beltCapacity: usize, // Capacity of EACH beltInventories
    pub beltInventories: Vec<Vec<Option<BeltItem>>>, // Vector of inventories, one per output lane
    pub beltClock: u128,
    pub beltTickSpeed: u128,

    pub producedCount: usize,
    pub consumedCount: usize,
    pub stateChangeCount: usize,
}
impl Machine
{
    pub fn new(id: usize, cost: usize, throughput: usize, state: OPCState, faultChance: f32, faultMessage: String,
            processingTickSpeed: u128, inputTickSpeed: u128, inputLanes: usize, inputInvCapacity: usize,
            outputTickSpeed: u128, outputInvCapacity: usize, outputLanes: usize, beltCapacity: usize, beltTickSpeed: u128) -> Self
    {
        let mut inIDs = Vec::<MachineLaneID>::new();
        inIDs.reserve(inputLanes);
        let inventories = vec![vec![None; beltCapacity]; outputLanes];

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
            processingInProgress: false,
            
            inputBehavior: None,
            inputClock: 0,
            inputTickSpeed,
            inputInProgress: false,
            inputWaiting: false,
            inputIDs: inIDs,
            inputInventory: 0,
            inputInvCapacity,
            nextInput: 0,
            
            outputBehavior: None,
            outputClock: 0,
            outputTickSpeed,
            outputInProgress: false,
            outputWaiting: false,
            outputInventory: 0,
            outputInvCapacity,
            nextOutput: 0,

            outputLanes,
            beltCapacity,
            beltInventories: inventories,
            beltClock: 0,
            beltTickSpeed,
            
            consumedCount: 0,
            producedCount: 0,
            stateChangeCount: 0,
        };

        return newMachine;
    }

    // Function for faulted state
    fn faulted(&mut self)
    {
        // println!("ID {}: {}", self.id, self.faultMessage); //now prints the fault message from JSON
        // TODO: unfaulting
    }

    fn checkIfShouldFault(&mut self, seed: i32) -> bool
    {
        // Modulo seed by 1000, convert to float, convert to % (out of 1000), and compare to fail chance
        if (seed % 1000) as f32 / 1000.0 < self.faultChance
        {
            // Debug logging to show the seed when the machine faults
            println!("ID {}: {}", self.id, self.faultMessage); // TODO: more than one fault type
            self.state = OPCState::FAULTED;
            self.stateChangeCount += 1;
            self.processingInProgress = false;
            self.inputInProgress = false;
            return true;
        }

        return false;
    }

    fn updateBelts(&mut self, deltaTime: u128)
    {
        for belt in self.beltInventories.iter_mut()
        {
            let len = belt.len();
            for i in 0 as usize..len - 1
            {
                // Get two mutable references, one to the (maybe) moving item,
                // and one to the destination
                let (head, tail) = belt.split_at_mut(i + 1);
                let item = head[i].as_mut();
                let nextItem = tail[0].as_mut();

                // If there is an item to move, unwrap
                if let Some(item) = item
                {
                    // If the item isn't moving, isn't at the end, and the next spot isn't occupied,
                    // start moving the item and zero its clock
                    if !item.isMoving && nextItem.is_none()
                    {
                        item.isMoving = true;
                        item.moveClock = 0;
                    }
                    else if !item.isMoving { continue; }

                    // Increment the item's movement clock, and continue
                    // if it is not done yet
                    item.moveClock += deltaTime;
                    if item.moveClock < item.tickSpeed { continue; }

                    // Movement is done, move the item up a place
                    item.isMoving = false;
                    belt[i + 1] = Some(item.to_owned());
                    belt[i] = None;
                }
            }
        }
    }

    fn checkBeltSupply(&mut self, id: usize) -> bool
    {
        // Check final spot on belt, as that is the spot waiting to be taken as input
        return self.beltInventories[id][self.beltCapacity - 1].is_some();
    }

    // // TODO: fix to work with new clock system
    // pub fn flowInput(&mut self, deltaTime: u128, mut machines: HashMap<usize, RefCell<Machine>>) -> bool
    // {
    //     // check if space in input and output inventories
    //     // if self.inputInventory >= self.inputInvCapacity
    //     // {   
    //     //     return false; 
    //     // }
      
    //     // for _i in 0 as usize..self.inputIDs.len()
    //     // {   
    //     //     let currentStructIDs = self.inputIDs[self.nextInput];
    //     //     // gets the machine of interest 
    //     //     let currentMachine = machines.get_mut(&currentStructIDs.machineID).expect("Value does not exist");
    //     //     // num of items on this belt
    //     //     let numOnBelt = currentMachine.beltInventories[currentStructIDs.laneID];

    //     //     // belt has something 
    //     //     if numOnBelt > 0
    //     //     {
    //     //         currentMachine.beltInventories[currentStructIDs.laneID] -= 1;
    //     //         self.inputInventory += 1;
    //     //         self.nextInput += 1;
    //     //         // stays the same, resets if out of bounds 
    //     //         self.nextInput = self.nextInput % self.inputIDs.len();
    //     //         return true;
                
    //     //     }

    //     //     self.nextInput += 1;
    //     //     self.nextInput = self.nextInput % self.inputIDs.len();
    //     // }

    //     return false;
    // }

    // // Processes if there is enough room in output, even if not empty
    // // TODO: fix with new clock system
    // pub fn flowProcessing(&mut self, deltaTime: u128, seed: i32) -> bool
    // {
    //     // // check if enough input 
    //     // if self.inputInventory < self.cost
    //     // { 
    //     //     if self.state != OPCState::STARVED
    //     //     {
    //     //         self.state = OPCState::STARVED;
    //     //         self.stateChangeCount += 1;
    //     //         println!("ID {}: Starved.", self.id);
    //     //     }
    //     //     return false;
    //     // }

    //     // // check if room to output if processed
    //     // if self.outputInvCapacity - self.outputInventory < self.throughput
    //     // {
    //     //     if self.state != OPCState::BLOCKED
    //     //     {
    //     //         self.state = OPCState::BLOCKED;
    //     //         self.stateChangeCount += 1;
    //     //         println!("ID {}: Blocked.", self.id);
    //     //     }
    //     //     return false;
    //     // }
        
    //     // if self.checkIfShouldFault(seed) { return false; }

    //     // // process 
    //     // self.inputInventory -= self.cost;
    //     // self.consumedCount += self.cost;

    //     // self.outputInventory += self.throughput;
    //     // self.producedCount += self.throughput;

    //     // if self.state != OPCState::PRODUCING
    //     // {
    //     //     self.state = OPCState::PRODUCING;
    //     //     println!("ID {}: Switched back to producing state and produced.", self.id);
    //     // }
    //     // else {
    //     //     println!("ID {}: Produced.", self.id);
    //     // }

    //     return true;
    // }
}
