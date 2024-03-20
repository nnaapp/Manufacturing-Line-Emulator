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
    pub faultChance: f32,
    pub faultMessage: String,
    pub faultTimeHigh: f32,
    pub faultTimeLow: f32,
    pub inputIDs: Vec<String>,
    pub inputBehavior: String,
    pub inputSpeed: u128, // ms
    pub inputCapacity: usize,
    pub processingBehavior: String,
    pub processingSpeed: u128,
    pub outputIDs: Vec<String>,
    pub outputBehavior: String,
    pub outputSpeed: u128,
    pub outputCapacity: usize,
    pub sensor: bool,
    pub baseline: f64, 
    pub variance: f64,    
}

#[derive(Clone, Debug, Deserialize)]
pub struct JSONConveyor {
    pub id: String,
    pub capacity: usize,
    pub beltSpeed: u128,
    pub inputID: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JSONFactory {
    pub name: String,
    pub description: String,
    pub simSpeed: f64,
    pub pollRate: u128,
    pub Runtime: u128,
    pub Machines: Vec<JSONMachine>,
    pub Conveyors: Vec<JSONConveyor>,
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
