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

    pub processingBehavior: Option<fn(&mut Machine, u128, i32) -> bool>, 
    pub processingClock: u128, // deltaTime is in milliseconds
    pub processingTickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    pub processingInProgress: bool,

    pub inputBehavior: Option<fn(&mut Machine, u128, &mut HashMap<usize, Machine>) -> bool>, // Function pointer that can also be None, used to define behavior
    pub inputClock: u128,
    pub inputTickSpeed: u128, // tick for pulling input into inputInventory
    pub inputInProgress: bool,
    pub inputWaiting: bool, // is there room for input, and input to be taken?
    pub inputIDs: Vec<MachineLaneID>, // Vector of machine/lane IDs for input, used as indices
    pub inputInventory: usize, // storage place in machine before process 
    pub inputInvCapacity: usize, 
    pub nextInput: usize, // the input lane to start checking from 

    pub outputBehavior: Option<fn(&mut Machine, u128) -> bool>,
    pub outputClock: u128, 
    pub outputTickSpeed: u128, // tick for outputting 
    pub outputInProgress: bool,
    pub outputWaiting: bool, // is there output in the machine, and room to spit it out?
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
            
            consumedCount: 0,
            producedCount: 0,
            stateChangeCount: 0,
        };

        return newMachine;
    }

    pub fn update(&mut self, deltaTime: u128, seed: i32, machines: &mut HashMap<usize, Machine>/*, input: &mut Belt, output: &mut Belt*/)
    {
        // Execute input
        // Input needs to manage: 
        //     inputInProgress
        //     inputWaiting
        //     inputClock
        if self.inputBehavior.is_none()
        {
            println!("ID {}: Input behavior is not defined.", self.id);
            self.faultMessage = format!("Simulation Error: input behavior not defined.");
            return;
        }
        let inputBehavior = self.inputBehavior.unwrap();
        inputBehavior(self, deltaTime, machines);
        
        // Execute processing 
        // Processing needs to manage:
        //     processingInProgress
        //     processingClock
        if self.processingBehavior.is_none()
        {
            println!("ID {}: Processing behavior is not defined.", self.id);
            self.faultMessage = format!("Simulation Error: processing behavior not defined.");
            return;
        }
        let processingBehavior = self.processingBehavior.unwrap();
        processingBehavior(self, deltaTime, seed);

        // Execute output
        // Output needs to manage:
        //     outputInProgress
        //     outputWaiting
        //     outputClock
        if self.outputBehavior.is_none()
        {
            println!("ID {}: Output behavior is not defined.", self.id);
            self.faultMessage = format!("Simulation Error: output behavior not defined.");
            return;
        }
        let outputBehavior = self.outputBehavior.unwrap();
        outputBehavior(self, deltaTime);
    }

    // Function for faulted state
    fn faulted(&mut self)
    {
        println!("ID {}: {}", self.id, self.faultMessage); //now prints the fault message from JSON
    }

    fn findInputSingle(&mut self, machines: &mut HashMap<usize, Machine>) -> bool
    {
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
            if numOnBelt > 0 { return true; }

            self.nextInput += 1;
            self.nextInput = self.nextInput % self.inputIDs.len();
        }

        return false;
    }

    fn findOutputSingle(&mut self) -> bool
    {
        if self.outputInventory <= 0
        {
            return false;
        }
        
        for _i in 0 as usize..self.outputLanes
        {
            if self.beltInventories[self.nextOutput] < self.beltCapacity { return true; }

            self.nextOutput += 1;
            self.nextOutput = self.nextOutput % self.beltInventories.len();
        }
        
        return false;
    }

    #[allow(unused_variables)]
    // Always has supply to input, like the start of a line
    pub fn spawnerInput(&mut self, deltaTime: u128, machines: &mut HashMap<usize, Machine>) -> bool
    {
        if !self.inputInProgress && (self.inputInventory < self.inputInvCapacity)
        {
            // Set the input to be in progress
            self.inputWaiting = true;
            self.inputInProgress = true;
            self.inputClock = 0;
        }

        if !self.inputInProgress 
        { 
            self.inputWaiting = false;
            return false; 
        }

        if self.inputClock < self.inputTickSpeed
        {
            self.inputClock += deltaTime;
            return false;
        }

        self.inputInventory += 1;
        self.inputInProgress = false;
        return true;        
    }

    // Inputs only ONE thing if output is empty,
    // ASSUMES that findOutputSingle has been called
    pub fn singleInput(&mut self, deltaTime: u128, machines: &mut HashMap<usize, Machine>) -> bool
    {
        if !self.inputInProgress && self.outputInventory <= 0 
        {
            if !self.findInputSingle(machines) { return false; }
            // gets the machine of interest 
            let currentMachineLaneID = self.inputIDs[self.nextInput];
            let currentMachine = machines.get_mut(&currentMachineLaneID.machineID).expect("Value does not exist");
            // Take 1 item off it (reserve so nothing else can take it, essentially)
            currentMachine.beltInventories[currentMachineLaneID.laneID] -= 1;
            // Increment nextInput for balanced taking of items
            self.nextInput += 1;
            self.nextInput = self.nextInput % self.inputIDs.len();

            // Set the input to be in progress
            self.inputWaiting = true;
            self.inputInProgress = true;
            self.inputClock = 0;
        }

        if !self.inputInProgress 
        { 
            self.inputWaiting = false;
            return false; 
        }

        if self.inputClock < self.inputTickSpeed
        {
            self.inputClock += deltaTime;
            return false;
        }

        self.inputInventory += 1;
        self.inputInProgress = false;
        return true;
    }

    // Inputs even if there is something in the output
    // TODO: fix to work with new clock system
    pub fn flowInput(&mut self, deltaTime: u128, machines: &mut HashMap<usize, Machine>) -> bool
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
    pub fn defaultProcessing(&mut self, deltaTime: u128, seed: i32) -> bool
    {
        if !self.processingInProgress && !self.inputWaiting && !self.outputWaiting
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
        }

        if !self.processingInProgress
            && self.inputInventory >= self.cost
            && self.outputInventory == 0 
            && self.outputInvCapacity >= self.throughput
        {
            self.processingInProgress = true;
            self.processingClock = 0;
        }

        if !self.processingInProgress { return false; }

        if self.processingClock < self.processingTickSpeed
        {
            self.processingClock += deltaTime;
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

        self.processingInProgress = false;
        return true;
    }

    // Processes if there is enough room in output, even if not empty
    // TODO: fix with new clock system
    pub fn flowProcessing(&mut self, deltaTime: u128, seed: i32) -> bool
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
    pub fn singleOutput(&mut self, deltaTime: u128) -> bool
    {
        if !self.outputInProgress && self.outputInventory > 0
        {
            if !self.findOutputSingle() { return false; }

            // Debating this one, unsure if this should be pre or post clock
            // self.outputInventory -= 1;
            self.outputWaiting = true;
            self.outputInProgress = true;
            self.outputClock = 0;
        }

        if !self.outputInProgress 
        { 
            self.outputWaiting = false;
            return false; 
        }

        if self.outputClock < self.outputTickSpeed
        {
            self.outputClock += deltaTime;
            return false;
        }

        self.outputInventory -= 1;
        self.beltInventories[self.nextOutput] += 1;

        self.nextOutput += 1;
        self.nextOutput = self.nextOutput % self.beltInventories.len();

        self.outputInProgress = false;
        return true;
    }

    // Outputs one thing onto EVERY lane at once
    // fn multipleOutput(&mut self) -> bool
    // {
    //     // TODO
    //     return true;
    // }

    // Always has space to output, like the end of a line
    pub fn consumerOutput(&mut self, deltaTime: u128) -> bool
    {
        if !self.outputInProgress && self.outputInventory > 0
        {
            self.outputWaiting = true;
            self.outputInProgress = true;
            self.outputClock = 0;
        }

        if !self.outputInProgress 
        { 
            self.outputWaiting = false;
            return false; 
        }

        if self.outputClock < self.outputTickSpeed
        {
            self.outputClock += deltaTime;
            return false;
        }

        self.outputInventory -= 1;
        self.outputInProgress = false;
        return true;
    }
}
