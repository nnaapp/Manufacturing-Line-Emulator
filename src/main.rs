#![allow(non_snake_case)]

use std::time::{UNIX_EPOCH,SystemTime,Duration};

struct Machine
{
    deltaTime: f64,
    tickSpeed: i64,
    failChance: f32,
}
impl Machine 
{
    fn new(tickSpeed: i64, failChance: f32) -> Self
    {
        return Self { deltaTime: 0.0, tickSpeed: tickSpeed, failChance: failChance };
    }

    fn update(&self)
    {
        println!("Update func");
    }
}

fn main() 
{
    let myMachine = Machine::new(5, 0.01);

    let mut start = SystemTime::now();
    let mut iterTime = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
    let mut prevTime = iterTime;
    let mut deltaTime:u128 = 0;
    loop
    {   
        start = SystemTime::now();
        iterTime = start.duration_since(UNIX_EPOCH).expect("Time went backwards");     
        deltaTime += iterTime.as_millis() - prevTime.as_millis();
        
        if deltaTime >= 1000
        {
            println!("Test");
            deltaTime -= 1000;
        }

        prevTime = iterTime;
    }
}
