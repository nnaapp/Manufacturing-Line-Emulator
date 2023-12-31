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
    state: OPCState,
}
impl Machine 
{
    fn new(tickSpeed: u128, failChance: f32, initialState: OPCState) -> Self
    {
        return Self { deltaTime: 0, tickSpeed, failChance, state: initialState };
    }

    fn update(&mut self, deltaTime: u128, seed: i32)
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
            OPCState::PRODUCING=>self.producing(seed),
            OPCState::FAULTED=>self.faulted(),
        }
    }

    // Function for producing state
    fn producing(&mut self, seed: i32)
    {
        println!("Producing.");
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

fn main() 
{
    let mut myMachine = Machine::new(500, 0.05, OPCState::PRODUCING);

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
        myMachine.update(deltaTime, rng.gen_range(0..=std::i32::MAX));

        // Log system time at the start of this iteration, for use in next iteration
        prevTime = iterTime;
    }
}
