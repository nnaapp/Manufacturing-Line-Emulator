#![allow(non_snake_case)]

use std::time::{UNIX_EPOCH,SystemTime,Duration};
extern crate rand;
use rand::{Rng, rngs::ThreadRng};

struct Machine
{
    deltaTime: u128, // deltaTime is in milliseconds
    tickSpeed: u128, // tickSpeed is in milliseconds, number of milliseconds between ticks
    failChance: f32,
}
impl Machine 
{
    fn new(tickSpeed: u128, failChance: f32) -> Self
    {
        return Self { deltaTime: 0, tickSpeed, failChance };
    }

    fn update(&mut self, deltaTime: u128, rng: &mut ThreadRng)
    {
        self.deltaTime += deltaTime;
        
        // If it is not time to execute a tick, return
        if self.deltaTime < self.tickSpeed
        {
            return;
        }

        // Execute a tick
        println!("Update func");
        self.deltaTime -= self.tickSpeed;
        if rng.gen_range(0.0..1.0) <= self.failChance
        {
            println!("Failure");
        }
    }
}

fn main() 
{
    let mut myMachine = Machine::new(500, 0.01);

    // Master random number generator, which is passed to machines to use for faults
    let mut rng = rand::thread_rng();

    // Start represents current SystemTime, 
    // iter/prevTime represent milliseconds since epoch time for the current and previous iteration of loop,
    // deltaTime represents milliseconds time between previous and current iteration of loop.
    let mut start = SystemTime::now();
    let mut iterTime:Duration = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    let mut prevTime:Duration = iterTime;
    let mut deltaTime:u128;
    loop
    {   
        start = SystemTime::now();
        iterTime = start.duration_since(UNIX_EPOCH).expect("Time went backwards");     

        deltaTime = iterTime.as_millis() - prevTime.as_millis();

        myMachine.update(deltaTime, &mut rng);

        prevTime = iterTime;
    }
}
