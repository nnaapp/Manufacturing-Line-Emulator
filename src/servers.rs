use local_ip_address::local_ip;

use opcua::server::prelude::*;

use std::path::PathBuf;
use std::sync::RwLock;
use std::fs::metadata;

use actix_web::{get, post, App, HttpResponse, HttpRequest, HttpServer, Responder, web, Result as ActixResult};
use actix_files::NamedFile;

use serde::{Serialize, Deserialize};

use crate::json;
use json::*;
use jsonschema::JSONSchema;

pub fn initOPCServer() -> Server
{
    let ipAddress = local_ip().expect("IP could not be found.");
    let hostName = hostname().expect("Hostname could not be found.");
    let discoveryURL = format!("opc.tcp://{ipAddress}:4855/");
    println!("Discovery URL: {}", discoveryURL);

    let server = ServerBuilder::new()
        .application_name("OPC UA Simulation Server")
        .application_uri("urn:OPC UA Simulation Server")
        .create_sample_keypair(true)
        .certificate_path(&PathBuf::from("own/cert.der"))
        .private_key_path(&PathBuf::from("private/private.pem"))
        .pki_dir("./pki-server")
        .discovery_server_url(None)
        .host_and_port(ipAddress.to_string(), 4855)
        .discovery_urls(vec![format!("/"), format!("opc.tcp://{hostName}:4855/")])
        .endpoints(
            [
                ("none", "/", SecurityPolicy::None, MessageSecurityMode::None, &["ANONYMOUS"]),
                ("basic128rsa15_sign", "/", SecurityPolicy::Basic128Rsa15, MessageSecurityMode::Sign, &["ANONYMOUS"]),
                ("basic128rsa15_sign_encrypt", "/", SecurityPolicy::Basic128Rsa15, MessageSecurityMode::SignAndEncrypt, &["ANONYMOUS"]),
                ("basic256_sign", "/", SecurityPolicy::Basic256, MessageSecurityMode::Sign, &["ANONYMOUS"]),
                ("basic256_sign_encrypt", "/", SecurityPolicy::Basic256, MessageSecurityMode::SignAndEncrypt, &["ANONYMOUS"]),
                ("basic256sha256_sign", "/", SecurityPolicy::Basic256Sha256, MessageSecurityMode::Sign, &["ANONYMOUS"]),
                ("basic256sha256_sign_encrypt", "/", SecurityPolicy::Basic256Sha256, MessageSecurityMode::SignAndEncrypt, &["ANONYMOUS"]),
            ].iter().map(|v| {
                (v.0.to_string(), ServerEndpoint::from((v.1, v.2, v.3, &v.4[..])))
            }).collect())
        .server().unwrap();
    return server;
}

// The four states for the simulation
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum SimulationState
{
    RUNNING, // Running as normal
    PAUSED,  // Paused but still waiting
    STOP,    // Full-stop the simulation
    EXIT,    // Fully exit the program
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// Start of the cursed lands //////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
// This is our solution to getting signals from our Actix HTTP server out into the simulator.
// We could not get an object or other easy to move flag inside the service that the web page uses,
// so we use static memory to keep track of a "master state" for the simulation, which is 
// either get or set depending on function arguments. The same is true for every funciton in this
// section, these are all middlemen between the server and the simulation for various values.
// 
// false and None for getter, true and Some(SimulationState::StateHere) for setter 
pub fn simStateManager(updateState: bool, newState: Option<SimulationState>) -> SimulationState
{
    static STATE: RwLock<SimulationState> = RwLock::new(SimulationState::STOP);

    if updateState && newState.is_some()
    {
        *STATE.write().unwrap() = newState.unwrap();
    }

    let state = *STATE.read().ok().unwrap();
    return state.clone();
}

pub fn simConfigManager(updateConfig: bool, newConfig: Option<String>) -> String
{
    static CONFIG: RwLock<String> = RwLock::new(String::new());

    if updateConfig && newConfig.is_some()
    {
        let newConfig = newConfig.unwrap();
        *CONFIG.write().unwrap() = newConfig;
    }

    if CONFIG.read().ok().unwrap().len() == 0
    {
        *CONFIG.write().unwrap() = String::from("factory.json");
    }

    return CONFIG.read().ok().unwrap().clone();
}

pub fn simClockManager(zeroTimes: bool, updateTimes: bool, deltaTime: Option<u128>) -> (u128, u128)
{
    static ACTIVETIME: RwLock<u128> = RwLock::new(0);
    static RUNTIME: RwLock<u128> = RwLock::new(0);

    let state = simStateManager(false, None);

    if zeroTimes
    {
        *RUNTIME.write().unwrap() = 0;
        *ACTIVETIME.write().unwrap() = 0;

        return (0, 0);
    }

    if updateTimes && deltaTime.is_some()
    {
        let deltaTime = deltaTime.unwrap();
        *RUNTIME.write().unwrap() += deltaTime;

        if state == SimulationState::RUNNING
        {
            *ACTIVETIME.write().unwrap() += deltaTime;
        }
    }

    let activetime = *ACTIVETIME.read().ok().unwrap();
    let runtime = *RUNTIME.read().ok().unwrap();
    return (activetime, runtime);
}

pub fn simTimerManager(updateTimer: bool, newTimer: Option<i128>) -> i128
{
    static TIME_LIMIT: RwLock<i128> = RwLock::new(0);

    if updateTimer && newTimer.is_some()
    {
        *TIME_LIMIT.write().unwrap() = newTimer.unwrap();
    }

    return TIME_LIMIT.read().ok().unwrap().clone()
}

///////////////////////////////////////////////////////////////////////////////////////////////////
// End of the cursed lands ////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

// Get HTML for web page
#[get("/")]
async fn getPage() -> impl Responder
{
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../data/static/index.html"))
}

#[derive(Serialize)]
struct MessageResponse
{
    message: String
}

// Stop or start the simulation but not the program
#[post("/toggleSim")]
async fn toggleSim() -> ActixResult<impl Responder>
{
    let state = simStateManager(false, None);

    if state == SimulationState::STOP
    {
        //////////////////////
        // Timer validation //
        if simTimerManager(false, None) < 0
        {
            return Ok(web::Json(MessageResponse {message: String::from("Time cannot be negative.")}));
        }
        //////////////////////
        
        ////////////////////////////
        // Config file validation //
        let mut configPath = simConfigManager(false, None);

        if in_container::in_container()
        {
            configPath = format!("/home/data/{}", configPath);
        }
        else
        {
            configPath = format!("./data/{}", configPath);
        }

        if let Err(_e) = metadata(configPath.clone())
        {
            return Ok(web::Json(MessageResponse {message: String::from("File does not exist.")}));
        }
        
        let jsonData = read_json_file(configPath.as_str());

        let data_as_value: serde_json::Value = serde_json::from_str(&jsonData).expect("Failed to parse JSON");

        // JSON file validation using JSON schema
        let schema_string = read_json_file("./data/schema.json");
        let schema_data = serde_json::from_str(&schema_string).expect("Failed to parse Schema");

        let compiled_schema = JSONSchema::compile(&schema_data).expect("Could not compile schema");

        let result = compiled_schema.validate(&data_as_value);
        if let Err(errors) = result {
            for error in errors {
                println!("Validation error: {}", error);
                println!("Instance path: {}", error.instance_path);
            }

            return Ok(web::Json(MessageResponse {message: String::from("JSON file has invalid structure.")}));
        }

        // If we get here, the JSON is correct and we can continue to turn the simulation on//
        //////////////////////////////////////////////////////////////////////////////////////
    }

    match state
    {
        SimulationState::RUNNING => simStateManager(true, Some(SimulationState::STOP)),
        SimulationState::STOP => simStateManager(true, Some(SimulationState::RUNNING)),
        SimulationState::EXIT => SimulationState::EXIT,
        _ => simStateManager(true, Some(SimulationState::STOP))
    };

    Ok(web::Json(MessageResponse {message: String::from("success")}))
}

#[derive(Serialize)]
struct StateQuery
{
    state: String
}

#[get("/simState")]
async fn getSimState() -> ActixResult<impl Responder>
{
    let mut stateJSON = StateQuery{ state: String::from("running") };
    let state = simStateManager(false, None);
    match state
    {
        SimulationState::RUNNING => stateJSON.state = String::from("running"),
        SimulationState::STOP => stateJSON.state = String::from("stop"),
        SimulationState::PAUSED => stateJSON.state = String::from("paused"),
        _ => stateJSON.state = String::from("error")
    }

    Ok(web::Json(stateJSON))
}

// Exit the program entirely
#[post("/exitSim")]
async fn exitSim() -> impl Responder
{
    simStateManager(true, Some(SimulationState::EXIT));

    HttpResponse::Ok()
}

// Pause or unpause the simulation without killing it fully
#[post("/suspendSim")]
async fn suspendSim() -> impl Responder
{
    match simStateManager(false, None)
    {
        SimulationState::RUNNING => simStateManager(true, Some(SimulationState::PAUSED)),
        SimulationState::PAUSED => simStateManager(true, Some(SimulationState::RUNNING)),
        SimulationState::EXIT => SimulationState::EXIT,
        SimulationState::STOP => SimulationState::STOP
    };

    HttpResponse::Ok()
}

#[derive(Deserialize)]
struct ConfigQuery 
{
    config: String
}

#[post("/setConfig")]
async fn setSimConfig(info: web::Query<ConfigQuery>) -> impl Responder
{
    simConfigManager(true, Some(info.config.clone()));

    HttpResponse::Ok()
}

#[derive(Serialize)]
struct TimeResponse
{
    activeTime: u128,
    runningTime: u128
}

#[get("/getTime")]
async fn getSimTime() -> ActixResult<impl Responder>
{
    let rawTimes = simClockManager(false, false, None);
    let timesObj = TimeResponse {
        activeTime: rawTimes.0,
        runningTime: rawTimes.1
    };

    Ok(web::Json(timesObj))
}

#[derive(Serialize)]
struct RemainingTimeResponse
{
    time: u128
}

#[get("/getTimeLimit")]
async fn getSimTimeLimit() -> ActixResult<impl Responder>
{
    let timeLimit = simTimerManager(false, None);
    if timeLimit <= 0
    {
        return Ok(web::Json(RemainingTimeResponse { time: 0 }));
    }

    let timePassed = simClockManager(false, false, None).0; // Time unpaused, does not track paused time, .0 gets this
    let timeLeft = timeLimit as u128 - timePassed;
    if timeLeft > timeLimit as u128
    {
        return Ok(web::Json(RemainingTimeResponse { time: 0 }));
    }

    Ok(web::Json(RemainingTimeResponse { time: timeLeft }))
}


#[derive(Deserialize)]
struct TimerQuery
{
    timer: i64
}

#[post("/setTimer")]
async fn setSimTimer(info: web::Query<TimerQuery>) -> impl Responder
{
    // Converts received time into microseconds, web service expects minutes
    simTimerManager(true, Some(info.timer.clone() as i128 * 1000000 * 60));
    HttpResponse::Ok()
}

async fn getLogo(_req: HttpRequest) -> ActixResult<NamedFile>
{
    Ok(NamedFile::open("./data/static/eosys.png")?)
}

// Set up and asynchronously run the Actix HTTP server for the control panel
#[actix_web::main]
pub async fn initWebServer() -> std::io::Result<()>
{
    let port = 8080;
    println!("Control Panel URL: http://{}:{}/", local_ip().expect("IP could not be found."), port);
    HttpServer::new(|| {
        App::new()
            .route("/eosys.png", web::get().to(getLogo))
            .service(getPage)
            .service(toggleSim)
            .service(exitSim)
            .service(suspendSim)
            .service(setSimConfig)
            .service(getSimTime)
            .service(getSimTimeLimit)
            .service(setSimTimer)
            .service(getSimState)
        })
        .disable_signals()
        .bind((local_ip().expect("IP could not be found."), port))?
        .run()
        .await
}
