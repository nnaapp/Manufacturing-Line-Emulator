extern crate serde_json;
extern crate serde;
use self::serde::Deserialize;

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use machine::MachineLaneID;

#[derive(Debug, Deserialize)]
pub struct JSONMachine {
    pub id: usize,
    pub cost: usize,
    pub throughput: usize,
    pub state: String,
    pub faultChance: f32,
    pub faultMessage: String,
    pub faultTimeHigh: f32,
    pub faultTimeLow: f32,
    pub inputIDs: Vec<MachineLaneID>,
    pub inputBehavior: String,
    pub inputSpeed: u128, // ms
    pub inputCapacity: usize,
    pub processingBehavior: String,
    pub processingSpeed: u128,
    pub outputBehavior: String,
    pub outputSpeed: u128,
    pub outputCapacity: usize,
    pub outputLanes: usize,
    pub beltCapacity: usize,
}

#[derive(Debug, Deserialize)]
pub struct JSONFactory {
    pub name: String,
    pub description: String,
    pub simSpeed: f64,
    pub pollRate: u128,
    pub Runtime: u128,
    pub Machines: Vec<JSONMachine>,
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
