use std::fmt;
use std::collections::HashMap;

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

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct MachineLaneID
{
    pub machineID: usize,
    pub laneID: usize,
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

    pub processingBehavior: Option<fn(&mut Machine, i32) -> bool>, 
    pub processingClock: u128, // deltaTime is in milliseconds
    pub processingTickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    pub processingTickHeld: bool,

    pub inputBehavior: Option<fn(&mut Machine, &mut HashMap<usize, Machine>) -> bool>, // Function pointer that can also be None, used to define behavior
    pub inputClock: u128,
    pub inputTickSpeed: u128, // tick for pulling input into inputInventory
    pub inputTickHeld: bool,
    pub inputIDs: Vec<MachineLaneID>, // Vector of machine/lane IDs for input, used as indices
    pub inputInventory: usize, // storage place in machine before process 
    pub inputInvCapacity: usize, 
    pub nextInput: usize, // the input lane to start checking from 

    pub outputBehavior: Option<fn(&mut Machine) -> bool>,
    pub outputClock: u128, 
    pub outputTickSpeed: u128, // tick for outputting 
    pub outputTickHeld: bool,
    pub outputInventory: usize, // represents num of items in it 
    pub outputInvCapacity: usize,
    pub nextOutput: usize, // the output lane to start checkng from

    pub outputLanes: usize, // number out output lanes
    pub beltCapacity: usize, // Capacity of EACH beltInventories
    pub beltInventories: Vec<usize>, // Vector of inventories, one per output lane

    pub producedCount: usize,
    pub consumedCount: usize,
    pub stateChangeCount: usize,
}
impl Machine
{
    pub fn new(id: usize, cost: usize, throughput: usize, state: OPCState, faultChance: f32, faultMessage: String,
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

    pub fn update(&mut self, deltaTime: u128, seed: i32, machines: &mut HashMap<usize, Machine>/*, input: &mut Belt, output: &mut Belt*/)
    {
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
    pub fn spawnerInput(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
        if self.inputInventory < self.inputInvCapacity
        {
            self.inputInventory += 1;
            return true;
        }

        return false;
    }

    // Inputs only if output is empty
    pub fn defaultInput(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
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
    pub fn flowInput(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
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
    pub fn defaultProcessing(&mut self, seed: i32) -> bool
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
    pub fn flowProcessing(&mut self, seed: i32) -> bool
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
    pub fn defaultOutput(&mut self) -> bool
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
    pub fn consumerOutput(&mut self) -> bool
    {
        if self.outputInventory > 0
        {
            self.outputInventory -= 1;
            return true;
        }

        return false;
    }
}
