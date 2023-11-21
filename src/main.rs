#![allow(non_snake_case)]

use std::time::{UNIX_EPOCH,SystemTime,Duration};

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

    fn update(&mut self)
    {
        println!("Update func");
        self.deltaTime -= self.tickSpeed;
    }
}

fn main() 
{
    let mut myMachine = Machine::new(500, 0.01);

    let mut start = SystemTime::now();
    let mut iterTime:Duration = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    let mut prevTime:Duration = iterTime;
    loop
    {   
        start = SystemTime::now();
        iterTime = start.duration_since(UNIX_EPOCH).expect("Time went backwards");     

        myMachine.deltaTime += iterTime.as_millis() - prevTime.as_millis();
        
        if myMachine.deltaTime >= myMachine.tickSpeed
        {
            myMachine.update();
        }

        prevTime = iterTime;
    }
}
