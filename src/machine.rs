use std::fmt;
use std::collections::HashMap;
use std::cell::RefCell;
use std::cell::RefMut;

extern crate serde_json;
extern crate serde;

extern crate log2;
use log2::*;

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

#[derive(Clone)]
pub struct BeltItem
{
    pub moveClock: u128, // clock for current movement
    pub tickSpeed: u128, // time it takes to perform a movement
    pub isMoving: bool,
}

pub struct ConveyorBelt
{
    pub id: String,
    pub capacity: usize,
    pub belt: Vec<Option<BeltItem>>,
    pub beltSpeed: u128,
    pub isInputIDSome: bool,
    pub inputID: Option<String>,
}
impl ConveyorBelt
{
    pub fn new(id: String, capacity: usize, beltSpeed: u128, inputID: Option<String>) -> ConveyorBelt
    {
        let belt = vec![None; capacity];
        let isInputIDSome = inputID.is_some();
        return ConveyorBelt { id, capacity, belt, beltSpeed, isInputIDSome, inputID };
    }

    pub fn update(&mut self, inputConveyor: Option<RefMut<ConveyorBelt>>, deltaTime: u128)
    {
        if self.isInputIDSome
        {
            let id = &self.id;
            self.takeInput(&mut inputConveyor.expect(format!("Conveyor {id}'s input conveyor does not exist.").as_str()));
        }
        
        let len = self.belt.len();
        for i in 0 as usize..len - 1
        {
            // Get two mutable references, one to the (maybe) moving item,
            // and one to the destination
            let (head, tail) = self.belt.split_at_mut(i + 1);
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
                self.belt[i + 1] = Some(item.to_owned());
                self.belt[i] = None;
            }
        }
    }

    pub fn takeInput(&mut self, inputConveyor: &mut RefMut<ConveyorBelt>) -> bool
    {
        // take input off optional input conveyor belt
        if !self.isStartSome() && inputConveyor.isEndSome()
        {
            inputConveyor.pullItem();
            self.pushItem();

            return true;
        }

        return false;
    }

    pub fn isStartSome(&mut self) -> bool
    {
        return self.belt[0].is_some();
    }

    pub fn isEndSome(&mut self) -> bool
    {
        let len = self.belt.len();
        return self.belt[len - 1].is_some();
    }

    pub fn pushItem(&mut self) -> bool
    {
        if !self.isStartSome()
        {
            self.belt[0] = Some(BeltItem { moveClock: 0, tickSpeed: self.beltSpeed, isMoving: false });
            return true;
        }

        return false;
    }

    pub fn pullItem(&mut self) -> bool
    {
        if self.isEndSome()
        {
            let len = self.belt.len();
            self.belt[len - 1] = None;
            return true;
        }

        return false;
    }
}

#[derive(Clone)]
pub struct Machine
{
    pub id: String,
    pub cost: usize, // Cost to produce
    pub throughput: usize, // How much gets produced
    pub state: OPCState,
    pub faultChance: f32,
    pub faultMessage: String, //string for fault messages 
    pub faultTimeHigh: f32,
    pub faultTimeLow: f32,
    pub faultTimeCurrent: u128,
    pub faultClock: u128,

    pub processingBehavior: Option<fn(&mut Machine, u128, i32) -> bool>, 
    pub processingClock: u128, // deltaTime is in milliseconds
    pub processingTickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    pub processingInProgress: bool,
    pub processingDebouncer: bool, // Debouncing mechanism, needs to be true TWICE to change state

    pub inputBehavior: Option<fn(&mut Machine, &mut HashMap<String, RefCell<ConveyorBelt>>, u128) -> bool>, // Function pointer that can also be None, used to define behavior
    pub inputClock: u128,
    pub inputTickSpeed: u128, // tick for pulling input into inputInventory
    pub inputInProgress: bool,
    pub inputDebouncer: bool, // Debouncing mechanism, needs to be true TWICE to change state
    pub inputWaiting: bool, // is there room for input, and input to be taken?
    pub inputIDs: Vec<String>, // Vector of machine/lane IDs for input, used as indices
    pub inputInventory: usize, // storage place in machine before process 
    pub inputInvCapacity: usize, 
    pub nextInput: usize, // the input lane to start checking from 

    pub outputBehavior: Option<fn(&mut Machine, &mut HashMap<String, RefCell<ConveyorBelt>>, u128) -> bool>,
    pub outputClock: u128, 
    pub outputTickSpeed: u128, // tick for outputting 
    pub outputInProgress: bool,
    pub outputDebouncer: bool, // Debouncing mechanism, needs to be true TWICE to change state
    pub outputWaiting: bool, // is there output in the machine, and room to spit it out?
    pub outputIDs: Vec<String>,
    pub outputInventory: usize, // represents num of items in it 
    pub outputInvCapacity: usize,
    pub nextOutput: usize, // the output lane to start checkng from

    pub producedCount: usize,
    pub consumedCount: usize,
    pub stateChangeCount: usize,
}
impl Machine
{
    //pub fn sensorSim(baseline: f64, variance: f64) -> f64
    //{
    //    let time = deltaTime;
        //is essentially the variance, dictates the lower and upper bounds the readings can go
    //    let amplitude = variance; 
        //adjust frequency as desired (100 is default, making it 50 would double the speed)
    //    let angFrequency = 2.0 * 3.14 / 25.0; 
        //causes the starting temp to be the lower bounds 
    //    let timeOffSet = -3.14 / 2.0;  
        //produces a sinusoidal waveform centered on baseline
    //    let flux = amplitude * (angFrequency * time + timeOffSet).sin(); 
        //Add fluctuation to baseline* to get the current sens reading
    //    let sensNum = baseline + flux;

        
    //Track initial and make it start at time of 0
    //    let initialSensNum = sensorSim(baseline, variance, 0.0);
    //    let mut sensNum = initialSensNum;

        //TODO: move from this iterative loop and implement this logic with the new clock system / tick system
    //    for time_step in time
    //    {
    //        let time_float = time_step as f64;
    //        let newSensNum = sensorSim(baseline, variance, time_float);
            //Make sure we are within bounds and reading changes by whole numbers
    //        if newSensNum >= baseline - variance && newSensNum <= baseline + variance 
    //        {
    //           sensNum = newSensNum.round();
    //        } 
    //        else
    //        {
    //            let change = rng.gen_range(-2.0..=2.0); //Random whole number change between -2 and 2
    //            sensNum += change;
    //            sensNum = sensNum.max(baseline - variance).min(baseline + variance);
    //        }
            //println!("Time: {}, Temperature: {}", time_step, sensNum);
    //    }
            
    //    return sensNum;             
    //}

    pub fn new(id: String, cost: usize, throughput: usize, state: OPCState, faultChance: f32, faultMessage: String,
            faultTimeHigh: f32, faultTimeLow: f32, processingTickSpeed: u128, inputTickSpeed: u128, inputInvCapacity: usize,
            outputTickSpeed: u128, outputInvCapacity: usize) -> Self
    {
        let inIDs = Vec::<String>::new();
        let outIDs = Vec::<String>::new();

        let newMachine = Machine {
            id,
            cost,
            throughput,
            state,
            faultChance,
            faultMessage,
            faultTimeHigh,
            faultTimeLow,
            faultTimeCurrent: 0,
            faultClock: 0,

            processingBehavior: None,
            processingClock: 0,
            processingTickSpeed,
            processingInProgress: false,
            processingDebouncer: false,
            
            inputBehavior: None,
            inputClock: 0,
            inputTickSpeed,
            inputInProgress: false,
            inputDebouncer: false,
            inputWaiting: false,
            inputIDs: inIDs,
            inputInventory: 0,
            inputInvCapacity,
            nextInput: 0,
            
            outputBehavior: None,
            outputClock: 0,
            outputTickSpeed,
            outputInProgress: false,
            outputDebouncer: false,
            outputWaiting: false,
            outputIDs: outIDs,
            outputInventory: 0,
            outputInvCapacity,
            nextOutput: 0,
            
            consumedCount: 0,
            producedCount: 0,
            stateChangeCount: 0
        };

        return newMachine;
    }

    pub fn update(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>, deltaTime: u128, seed: i32)
    {
        {
            if self.state != OPCState::FAULTED
            {
                // Execute input
                // Input needs to manage: 
                //     inputInProgress
                //     inputWaiting
                //     inputClock
                if self.inputBehavior.is_none()
                {
                    error!("ID {}: Input behavior is not defined.", self.id);
                    self.faultMessage = format!("Simulation Error: input behavior not defined.");
                    return;
                }
                let inputBehavior = self.inputBehavior.unwrap();
                inputBehavior(self, conveyors, deltaTime);
            }
        }

        {
            if self.state != OPCState::FAULTED
            {
                // Execute processing 
                // Processing needs to manage:
                //     processingInProgress
                //     processingClock
                if self.processingBehavior.is_none()
                {
                    error!("ID {}: Processing behavior is not defined.", self.id);
                    self.faultMessage = format!("Simulation Error: processing behavior not defined.");
                    return;
                }
                let processingBehavior = self.processingBehavior.unwrap();
                processingBehavior(self, deltaTime, seed);
            }
        }

        {
            if self.state == OPCState::FAULTED
            {
                self.faulted(deltaTime);
            }
        }

        {
            // Execute output
            // Output needs to manage:
            //     outputInProgress
            //     outputWaiting
            //     outputClock
            if self.outputBehavior.is_none()
            {
                error!("ID {}: Output behavior is not defined.", self.id);
                self.faultMessage = format!("Simulation Error: output behavior not defined.");
                return;
            }
            let outputBehavior = self.outputBehavior.unwrap();
            outputBehavior(self, conveyors, deltaTime);
        }
    }

    // Function for faulted state
    fn faulted(&mut self, deltaTime: u128)
    {
        // println!("ID {}: {}", self.id, self.faultMessage); //now prints the fault message from JSON
        self.faultClock += deltaTime;
        if self.faultClock < self.faultTimeCurrent 
        {
            return;
        }
        self.state = OPCState::PRODUCING;
        info!("ID {} : Has been fixed: Producing Again.", self.id);
    }

    fn checkIfShouldFault(&mut self, seed: i32) -> bool
    {
        // Modulo seed by 1000, convert to float, convert to % (out of 1000), and compare to fail chance
        if (seed % 1000) as f32 / 1000.0 < self.faultChance
        {
            // Debug logging to show the seed when the machine faults
            debug!("ID {}: {}", self.id, self.faultMessage); // TODO: more than one fault type
            self.state = OPCState::FAULTED;
            self.stateChangeCount += 1;
            self.processingInProgress = false;
            self.inputInProgress = false;
            let midTimePercent = (seed % 101) as f32 / 100.0; //turn seed into percentage
            self.faultTimeCurrent = ((self.faultTimeHigh - self.faultTimeLow) * midTimePercent + self.faultTimeLow) as u128; //sets fault time to the a percent of the way between the low and high values.
            self.faultClock = 0;
            return true;
        }

        return false;
    }

    pub fn updateState(&mut self)
    {
        if self.state == OPCState::FAULTED
        {
            return;
        }
        
        // Check for problems on this machine, like blocked or starved
        if !self.processingInProgress// && !self.inputWaiting && !self.outputWaiting
        {
            // check if enough input 
            if self.inputInventory < self.cost && !self.inputWaiting
            { 
                if self.state != OPCState::STARVED && self.inputDebouncer == true
                {
                    self.state = OPCState::STARVED;
                    self.inputDebouncer = false;
                    self.stateChangeCount += 1;
                    info!("ID {}: Starved.", self.id);
                    return;
                }

                if self.inputDebouncer == false
                {
                    self.inputDebouncer = true;
                }
                
                return;
            }

            // check if room to output if processed
            if (self.outputInventory != 0 || self.outputInvCapacity < self.throughput) && !self.outputWaiting
            {
                if self.state != OPCState::BLOCKED && self.outputDebouncer == true
                {
                    self.state = OPCState::BLOCKED;
                    self.outputDebouncer = false;
                    self.stateChangeCount += 1;
                    info!("ID {}: Blocked.", self.id);
                }

                if self.outputDebouncer == false
                {
                    self.outputDebouncer = true;
                }
                
                return;
            }
        }

        // Nothing else happened, so we must be producing (no problems on this machine)
        if self.state != OPCState::PRODUCING && self.processingDebouncer == true
        {
            self.state = OPCState::PRODUCING;
            self.processingDebouncer = false;
            self.stateChangeCount += 1;
            info!("ID {}: Producing.", self.id);
            return;
        }

        if self.processingDebouncer == false
        {
            self.processingDebouncer = true;
        }

        return;
    }
    
    fn findInputSingle(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>) -> bool
    {
        if self.inputInventory >= self.inputInvCapacity
        {   
            return false; 
        }

        for _i in 0 as usize..self.inputIDs.len()
        {   
            let currentInputID = &self.inputIDs[self.nextInput];
            // gets the conveyor of interest 
            let mut currentConveyor = 
                conveyors.get(currentInputID)
                        .expect(format!("Conveyor {currentInputID} does not exist.").as_str())
                        .borrow_mut();
            // belt has something 
            if currentConveyor.isEndSome() == true { return true; }

            self.nextInput += 1;
            self.nextInput = self.nextInput % self.inputIDs.len();
        }

        return false;
    }

    fn findOutputSingle(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>) -> bool
    {
        if self.outputInventory <= 0
        {
            return false;
        }

        for _i in 0 as usize..self.outputIDs.len()
        {
            let currentOutputID = &self.outputIDs[self.nextOutput];
            let mut currentConveyor = 
                conveyors.get(currentOutputID)
                        .expect(format!("Conveyor {currentOutputID} does not exist.").as_str())
                        .borrow_mut();
            if currentConveyor.isStartSome() == false { return true; }

            self.nextOutput += 1;
            self.nextOutput = self.nextOutput % self.outputIDs.len();
        }
        
        return false;
    }

    #[allow(unused_variables)]
    // Always has supply to input, like the start of a line
    pub fn spawnerInput(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>, deltaTime: u128) -> bool
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
    pub fn singleInput(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>, deltaTime: u128) -> bool
    {
        if !self.inputInProgress && self.outputInventory <= 0 
        {
            if !self.findInputSingle(conveyors) 
            {
                self.inputWaiting = false; 
                return false; 
            }
            // gets the self of interest 
            let currentInputID = &self.inputIDs[self.nextInput];
            let mut currentConveyor = 
                conveyors.get(currentInputID)
                        .expect(format!("Conveyor {currentInputID} does not exist.").as_str())
                        .borrow_mut();
            // Take 1 item off it (reserve so nothing else can take it, essentially)
            currentConveyor.pullItem();
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

    // Processess only if the output inventory is empty
    pub fn defaultProcessing(&mut self, deltaTime: u128, seed: i32) -> bool
    {
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

        if self.checkIfShouldFault(seed) { return false; }
        
        // process 
        self.inputInventory -= self.cost;
        self.consumedCount += self.cost;

        self.outputInventory += self.throughput;
        self.producedCount += self.throughput;

        info!("ID {}: Produced.", self.id);

        self.processingInProgress = false;
        return true;
    }

    // Outputs one thing onto one lane at a time
    pub fn singleOutput(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>, deltaTime: u128) -> bool
    {
        if !self.outputInProgress && self.outputInventory > 0
        {
            if !self.findOutputSingle(conveyors) 
            {
                self.outputWaiting = false; 
                return false; 
            }
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
        let currentOutputID = &self.outputIDs[self.nextOutput];
        let mut currentConveyor = 
            conveyors.get(currentOutputID)
                    .expect(format!("Conveyor {currentOutputID} does not exist.").as_str())
                    .borrow_mut();
        currentConveyor.pushItem();
        // self.beltInventories[nextOutput][0] = Some(BeltItem { moveClock: 0, tickSpeed: self.beltTickSpeed, isMoving: false });

        self.nextOutput += 1;
        self.nextOutput = self.nextOutput % self.outputIDs.len();

        self.outputInProgress = false;
        return true;
    }

    // Always has space to output, like the end of a line
    #[allow(unused_variables)]
    pub fn consumerOutput(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>, deltaTime: u128) -> bool
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
