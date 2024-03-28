use serde::Deserialize;

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct JSONMachine {
    pub id: String,
    pub cost: usize,
    pub throughput: usize,
    pub state: String,
    pub faults: Vec<JSONFault>,
    pub inputIDs: Vec<String>,
    pub inputBehavior: String,
    pub inputSpeedMs: u128, // ms
    pub inputCapacity: usize,
    pub processingBehavior: String,
    pub processingSpeedMs: u128,
    pub outputIDs: Vec<String>,
    pub outputBehavior: String,
    pub outputSpeedMs: u128,
    pub outputCapacity: usize,
    pub sensor: bool,
    pub sensorBaseline: f64, 
    pub sensorVariance: f64,    
}

#[derive(Clone, Debug, Deserialize)]
pub struct JSONFault
{
    pub faultChance: f32, // percent chance for a fault
    pub faultMessage: String, // string for fault message
    pub faultTimeHighSec: f32,
    pub faultTimeLowSec: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JSONConveyor {
    pub id: String,
    pub capacity: usize,
    pub beltSpeedMs: u128,
    pub inputID: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JSONFactory {
    pub name: String,
    pub description: String,
    pub simSpeed: f64,
    pub pollRateMs: u128,
    pub debounceRateInPolls: i32,
    pub runtimeSec: u128,
    pub machines: Vec<JSONMachine>,
    pub conveyors: Vec<JSONConveyor>,
}

#[derive(Debug, Deserialize)]
pub struct JSONData {
    pub factory: JSONFactory,
}

pub fn read_json_file(file_path: &str) -> String {
    let mut file_content = String::new();
    let mut file = File::open(&PathBuf::from(file_path)).expect("Failed to open file");
    file.read_to_string(&mut file_content).expect("Failed to read file content");
    file_content
}
