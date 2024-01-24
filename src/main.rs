#![allow(non_snake_case)]

use std::time::{UNIX_EPOCH,SystemTime,Duration};
extern crate rand;
use rand::Rng;

#[derive(PartialEq, Eq)]
enum OPCState
{
    PRODUCING,
    FAULTED,
}

struct Machine
{
    deltaTime: u128, // deltaTime is in milliseconds
    tickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    failChance: f32,
    cost: i32,
    throughput: i32,
    state: OPCState,
    inputID: usize,
    outputID: usize,
}
impl Machine
{
    fn new(tickSpeed: u128, failChance: f32, cost: i32, throughput: i32, initialState: OPCState, inputID: usize, outputID: usize) -> Self
    {
        return Self { deltaTime: 0, tickSpeed, failChance, cost, throughput, state: initialState, inputID, outputID };
    }

    fn update(&mut self, deltaTime: u128, seed: i32, belts: &mut Vec<Belt>)
    {
        self.deltaTime += deltaTime;
        
        // If it is not time to execute a tick, return
        if self.deltaTime < self.tickSpeed
        {
            return;
        }

        // Execute a tick
        self.deltaTime -= self.tickSpeed;
        match self.state
        {
            OPCState::PRODUCING=>self.producing(seed, belts),
            OPCState::FAULTED=>self.faulted(),
        }
    }

    // Function for producing state
    fn producing(&mut self, seed: i32, belts: &mut Vec<Belt>)
    {
        println!("Producing.");
        if belts[self.inputID].spawner
        {
            belts[self.outputID].push(self.throughput);
        }
        else if belts[self.inputID].check_empty(self.cost)
        {
            belts[self.inputID].pop(self.cost);
            belts[self.outputID].push(self.throughput);
        }
        println!("{}", belts[self.outputID].count);

        // Modulo seed by 1000, convert to float, convert to % (out of 1000), and compare to fail chance
        if (seed % 1000) as f32 / 1000.0 <= self.failChance
        {
            // Debug logging to show the seed when the machine faults
            println!("{} {} {}", seed, seed % 1000, self.failChance);
            self.state = OPCState::FAULTED;
        }
    }

    // Function for faulted state
    fn faulted(&mut self)
    {
        println!("Faulted.");
    }
}

struct Belt
{
    spawner: bool, // If a belt is a spawner, it has infinite resources
    capacity: i32,
    count: i32,
    // input: Option<Box<Machine>>,
    // output: Option<Box<Machine>>,
}
impl Belt
{
    fn new(spawner: bool, capacity: i32) -> Self
    {
        return Self { spawner, capacity, count: 0 };
    }

    // True = not full, False = full
    fn check_full(&self, amount: i32) -> bool
    {
        return self.count + amount <= self.capacity;
    }

    // True = not empty, False = empty
    fn check_empty(&self, amount: i32) -> bool
    {
        return self.count - amount >= 0;
    }

    // Returns the overflow, items that would not fit on the belt
    fn push(&mut self, amount: i32) -> i32
    {
        if self.check_full(amount)
        {
            self.count += amount;
            return 0;
        }
        
        let overflow = (self.count + amount) - self.capacity;
        self.count += amount - overflow;
        return overflow;
    }

    // Returns the amount of items popped off the belt
    fn pop(&mut self, amount: i32) -> i32
    {
        if self.check_empty(amount)
        {
            self.count -= amount;
            return amount;
        }

        let underflow = self.count;
        self.count = 0;
        return underflow;
    }
}

fn main() 
{
    let mut machines = Vec::with_capacity(2);
    machines.push(Machine::new(750, 0.05, 1, 1, OPCState::PRODUCING, 0, 1));
    machines.push(Machine::new(750, 0.05, 1, 1, OPCState::PRODUCING, 1, 2));
    
    let mut belts = Vec::with_capacity(3);
    belts.push(Belt::new(true, 999));
    belts.push(Belt::new(false, 5));
    belts.push(Belt::new(false, 999));

    // Master random number generator, which is passed to machines to use for faults
    let mut rng = rand::thread_rng();

    // Start represents current SystemTime, 
    // iter/prevTime represent milliseconds since epoch time for the current and previous iteration of loop,
    // deltaTime represents milliseconds time between previous and current iteration of loop.
    let mut start = SystemTime::now();
    let mut iterTime:Duration = start.duration_since(UNIX_EPOCH).expect("Get epoch time in ms");
    let mut prevTime:Duration = iterTime;
    let mut deltaTime:u128;

    loop
    {   
        // Find deltatime between loop iterations
        start = SystemTime::now();
        iterTime = start.duration_since(UNIX_EPOCH).expect("Get epoch time in ms");     
        deltaTime = iterTime.as_millis() - prevTime.as_millis();

        // rng is used to seed the update with any random integer, which is used for any rng dependent operations
        for i in 0..2
        {
            machines[i].update(deltaTime, rng.gen_range(0..=std::i32::MAX), &mut belts);
        }

        // Log system time at the start of this iteration, for use in next iteration
        prevTime = iterTime;
    }
}
