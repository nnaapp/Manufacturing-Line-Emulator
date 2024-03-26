use std::fmt;
use std::collections::HashMap;
use std::cell::RefCell;
use std::cell::RefMut;

use rand::Rng;

use log2::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OPCState
{
    PRODUCING,
    FAULTED,
    BLOCKED,
    STARVED,
    STARVEDBLOCKED,
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
            OPCState::STARVEDBLOCKED => write!(f, "starved and blocked"),
        }
    }
}

#[derive(Clone)]
pub struct BeltItem
{
    pub moveClockUs: u128, // clock for current movement, in microseconds
    pub tickSpeedUs: u128, // time it takes to perform a movement, microseconds
    pub isMoving: bool,
}

pub struct ConveyorBelt
{
    pub id: String,
    pub capacity: usize,
    pub belt: Vec<Option<BeltItem>>,
    pub beltSpeedUs: u128, // time it takes to move one space on the belt, microseconds
    pub isInputIDSome: bool,
    pub inputID: Option<String>,
}
impl ConveyorBelt
{
    // Expects ID string, capacity of the belt, movement speed in microseconds per movement, and an Option for if the belt takes from another belt
    pub fn new(id: String, capacity: usize, beltSpeedUs: u128, inputID: Option<String>) -> ConveyorBelt
    {
        let belt = vec![None; capacity];
        let isInputIDSome = inputID.is_some();
        return ConveyorBelt { id, capacity, belt, beltSpeedUs, isInputIDSome, inputID };
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
                    item.moveClockUs = 0;
                }
                else if !item.isMoving { continue; }

                // Increment the item's movement clock, and continue
                // if it is not done yet
                item.moveClockUs += deltaTime;
                if item.moveClockUs < item.tickSpeedUs { continue; }

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
            self.belt[0] = Some(BeltItem { moveClockUs: 0, tickSpeedUs: self.beltSpeedUs, isMoving: false });
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
pub struct Fault
{
    pub faultChance: f32, // percent chance for a fault
    pub faultMessage: String, // string for fault message
    pub faultTimeHighSec: f32, // highest time the fault can stay, in seconds
    pub faultTimeLowSec: f32, // lowest time the fault  can stay, in seconds
}

#[derive(Clone)]
pub struct Machine
{
    pub id: String,
    pub cost: usize, // Cost to produce
    pub throughput: usize, // How much gets produced
    pub state: OPCState,
    pub faults: Vec<Fault>,
    pub currentFault: Option<Fault>,
    pub faultTimeCurrentUs: u128, // time that needs to pass for the fault to end, in microseconds
    pub faultClockUs: u128, // current time that has passed since the fault started, in microseconds

    pub processingBehavior: Option<fn(&mut Machine, u128) -> bool>, 
    pub processingClockUs: u128, // change in time since the processing started, in microseconds
    pub processingTickSpeedUs: u128, // how much time processing takes, in microseconds
    pub processingInProgress: bool,
    pub processingDebouncer: bool, // Debouncing mechanism, needs to be true TWICE to change state

    pub inputBehavior: Option<fn(&mut Machine, &mut HashMap<String, RefCell<ConveyorBelt>>, u128) -> bool>, // Function pointer that can also be None, used to define behavior
    pub inputClockUs: u128, // change in time since input started, in microseconds
    pub inputTickSpeedUs: u128, // how much time input takes, in microseconds
    pub inputInProgress: bool,
    pub inputDebouncer: bool, // Debouncing mechanism, needs to be true TWICE to change state
    pub inputWaiting: bool, // is there room for input, and input to be taken?
    pub inputIDs: Vec<String>, // Vector of machine/lane IDs for input, used as indices
    pub inputInventory: usize, // storage place in machine before process 
    pub inputInvCapacity: usize, 
    pub nextInput: usize, // the input lane to start checking from 

    pub outputBehavior: Option<fn(&mut Machine, &mut HashMap<String, RefCell<ConveyorBelt>>, u128) -> bool>,
    pub outputClockUs: u128, // change in time since output started, in microseconds
    pub outputTickSpeedUs: u128, // how much time output takes, in microseconds
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

    pub sensor: bool,
    pub baseline: f64,
    pub variance: f64
}
impl Machine
{
    pub fn sensor_Sim(baseline: f64, variance: f64) -> f64
    {
        let mut rng = rand::thread_rng();

        let change = rng.gen_range(-(variance/2.0)..=(variance/2.0)); //Random whole number change between the - half of variance and half of variance
        
        let sensNum = baseline + change;
        //This was used in testing to make sure this function worked, currently hovers around baseline and changes within the range
        //of half of variance so that the data isnt bouncing directly from high end to low end
        //println!("Temperature: {:.2}", sensNum);

        return sensNum;
    }

    pub fn new(id: String, cost: usize, throughput: usize, state: OPCState, faults: Vec<Fault>, 
            processingTickSpeedUs: u128, inputTickSpeedUs: u128, inputInvCapacity: usize,
            outputTickSpeedUs: u128, outputInvCapacity: usize, sensor: bool, baseline: f64, variance: f64) -> Self
    {
        let inIDs = Vec::<String>::new();
        let outIDs = Vec::<String>::new();

        let newMachine = Machine {
            id,
            cost,
            throughput,
            state,
            faults,
            currentFault: None,
            faultTimeCurrentUs: 0,
            faultClockUs: 0,

            processingBehavior: None,
            processingClockUs: 0,
            processingTickSpeedUs,
            processingInProgress: false,
            processingDebouncer: false,
            
            inputBehavior: None,
            inputClockUs: 0,
            inputTickSpeedUs,
            inputInProgress: false,
            inputDebouncer: false,
            inputWaiting: false,
            inputIDs: inIDs,
            inputInventory: 0,
            inputInvCapacity,
            nextInput: 0,
            
            outputBehavior: None,
            outputClockUs: 0,
            outputTickSpeedUs,
            outputInProgress: false,
            outputDebouncer: false,
            outputWaiting: false,
            outputIDs: outIDs,
            outputInventory: 0,
            outputInvCapacity,
            nextOutput: 0,

            sensor,
            baseline,
            variance,
            
            consumedCount: 0,
            producedCount: 0,
            stateChangeCount: 0,

            
        };

        return newMachine;
    }

    pub fn update(&mut self, conveyors: &mut HashMap<String, RefCell<ConveyorBelt>>, deltaTime: u128)
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
                    return;
                }
                let processingBehavior = self.processingBehavior.unwrap();
                processingBehavior(self, deltaTime);
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
        self.faultClockUs += deltaTime;
        if self.faultClockUs < self.faultTimeCurrentUs 
        {
            return;
        }
        self.state = OPCState::PRODUCING;
        self.currentFault = None;
        self.faultTimeCurrentUs = 0;
        self.faultClockUs = 0;
        info!("ID {} : Has been fixed: Producing Again.", self.id);
    }

    fn checkIfShouldFault(&mut self) -> bool
    {
        for fault in &self.faults {
            // Generate random value between 0 and 1000, used for determining if a fault happens
            let faultSeed = rand::thread_rng().gen_range(0..1001);
            if faultSeed as f32 / 1000.0 < fault.faultChance
            {
                self.currentFault = Some(fault.clone());
                // Debug logging to show a message when the machine faults
                debug!("ID {}: {}", self.id, fault.faultMessage); // TODO: more than one fault type
                self.state = OPCState::FAULTED;
                self.stateChangeCount += 1;
                self.processingInProgress = false;
                self.inputInProgress = false;
                // Generate another random value, used for determining how long the fault will stay
                let timeSeed = rand::thread_rng().gen_range(0..101);
                let midTimePercent = timeSeed as f32 / 100.0; //turn seed into percentage
                self.faultTimeCurrentUs = ((fault.faultTimeHighSec - fault.faultTimeLowSec) * midTimePercent + fault.faultTimeLowSec) as u128 * 1000 * 1000; //sets fault time to the a percent of the way between the low and high values.
                self.faultClockUs = 0;
                return true;
            }
        }
        return false;
    }

    pub fn updateState(&mut self)
    {
        if self.state == OPCState::FAULTED
        {
            return;
        }

        let mut stateNotProducing = false;

        // Check for problems on this machine, like blocked or starved
        if !self.processingInProgress
        {
            // not enough input 
            if self.inputInventory < self.cost && !self.inputWaiting
            {  
                stateNotProducing = true;

                if self.state == OPCState::PRODUCING && self.inputDebouncer == true
                {
                    self.state = OPCState::STARVED;
                    self.stateChangeCount += 1;
                    info!("ID {}: Starved.", self.id);
                }
                else if self.state == OPCState::BLOCKED && self.inputDebouncer == true
                {
                    self.state = OPCState::STARVEDBLOCKED;
                    self.stateChangeCount += 1;
                    info!("ID {}: Starved and Blocked", self.id);
                }
                
                if self.inputDebouncer == true
                {
                    self.inputDebouncer = false;
                }

                if self.inputDebouncer == false && (self.state == OPCState::PRODUCING || self.state == OPCState::BLOCKED)
                {
                    self.inputDebouncer = true;
                }
            }
            // enough input, remove starved state
            else
            {
                if self.state == OPCState::STARVEDBLOCKED && self.inputDebouncer == true
                {
                    self.state = OPCState::BLOCKED;
                    self.inputDebouncer = false;
                    self.stateChangeCount += 1;
                    info!("ID {}: Blocked", self.id);
                }

                if self.inputDebouncer == false && self.state == OPCState::STARVEDBLOCKED
                {
                    self.inputDebouncer = true;
                }
            }

            // check if room to output if processed
            if (self.outputInventory != 0 || self.outputInvCapacity < self.throughput) && !self.outputWaiting
            {
                stateNotProducing = true;
                
                if self.state == OPCState::PRODUCING && self.outputDebouncer == true
                {
                    self.state = OPCState::BLOCKED;
                    self.stateChangeCount += 1;
                    info!("ID {}: Blocked.", self.id);
                }
                else if self.state == OPCState::STARVED && self.outputDebouncer == true
                {
                    self.state = OPCState::STARVEDBLOCKED;
                    self.stateChangeCount += 1;
                    info!("ID {}: Starved and Blocked", self.id);
                }

                if self.outputDebouncer == true
                {
                    self.outputDebouncer = false;
                }

                if self.outputDebouncer == false && (self.state == OPCState::PRODUCING || self.state == OPCState::STARVED)
                {
                    self.outputDebouncer = true;
                }
            }
            // enough output room, get out of blocked state
            else 
            {
                //println!("ID {}: {} {}", self.id, self.state, self.outputWaiting);
                if self.state == OPCState::STARVEDBLOCKED && self.outputDebouncer == true
                {
                    self.state = OPCState::STARVED;
                    self.outputDebouncer = false;
                    self.stateChangeCount += 1;
                    info!("ID {}: Starved.", self.id);
                }
   
                if self.outputDebouncer == false && self.state == OPCState::STARVEDBLOCKED
                {
                    self.outputDebouncer = true;
                }
            }
        }

        // We should not go producing
        if stateNotProducing == true { return; }

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
            self.inputClockUs = 0;
        }

        if !self.inputInProgress 
        { 
            self.inputWaiting = false;
            return false; 
        }

        if self.inputClockUs < self.inputTickSpeedUs
        {
            self.inputClockUs += deltaTime;
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
            self.inputClockUs = 0;
        }
        else if self.outputInventory > 0 && self.findInputSingle(conveyors)
        {
            self.inputWaiting = true;
        }
        else
        {
            self.inputWaiting = false;
        }

        if !self.inputInProgress 
        { 
            return false; 
        }

        if self.inputClockUs < self.inputTickSpeedUs
        {
            self.inputClockUs += deltaTime;
            return false;
        }

        self.inputInventory += 1;
        self.inputInProgress = false;
        return true;
    }

    // Processess only if the output inventory is empty
    pub fn defaultProcessing(&mut self, deltaTime: u128) -> bool
    {
        if !self.processingInProgress
        {
            if self.inputInventory >= self.cost && self.outputInventory == 0 && self.outputInvCapacity >= self.throughput
            { 
                self.processingInProgress = true;
                self.processingClockUs = 0;
            }
            else
            {
                self.processingInProgress = false;
            }
        }

        if !self.processingInProgress { return false; }

        if self.processingClockUs < self.processingTickSpeedUs
        {
            self.processingClockUs += deltaTime;
            return false;
        }

        if self.checkIfShouldFault() { return false; }
        
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
            self.outputClockUs = 0;
        }

        if !self.outputInProgress 
        { 
            self.outputWaiting = false;
            return false; 
        }

        if self.outputClockUs < self.outputTickSpeedUs
        {
            self.outputClockUs += deltaTime;
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
            self.outputClockUs = 0;
        }

        if !self.outputInProgress 
        { 
            self.outputWaiting = false;
            return false; 
        }

        if self.outputClockUs < self.outputTickSpeedUs
        {
            self.outputClockUs += deltaTime;
            return false;
        }

        self.outputInventory -= 1;
        self.outputInProgress = false;
        return true;
    }
}
