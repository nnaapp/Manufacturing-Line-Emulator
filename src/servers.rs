use local_ip_address::local_ip;

use opcua::server::prelude::*;

use log2::*;

use std::path::PathBuf;
use std::sync::RwLock;

use actix_web::{get, post, App, HttpResponse, HttpServer, Responder, web, Result as ActixResult};

use serde::{Serialize, Deserialize};

pub fn initOPCServer() -> Server
{
    let ipAddress = local_ip().expect("IP could not be found.");
    let hostName = hostname().expect("Hostname could not be found.");
    let discoveryURL = format!("opc.tcp://{ipAddress}:4855/");
    info!("Discovery URL: {}", discoveryURL);

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

// This is our solution to getting signals from our Actix HTTP server out into the simulator.
// We could not get an object or other easy to move flag inside the service that the web page uses,
// so we use static memory to keep track of a "master state" for the simulation, which is 
// either get or set depending on function arguments.
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

pub fn simTimerManager(updateTimer: bool, newTimer: Option<u128>) -> u128
{
    static TIME_LIMIT: RwLock<u128> = RwLock::new(0);

    if updateTimer && newTimer.is_some()
    {
        *TIME_LIMIT.write().unwrap() = newTimer.unwrap();
    }

    return TIME_LIMIT.read().ok().unwrap().clone()
}

// Get HTML for web page
#[get("/")]
async fn getPage() -> impl Responder
{
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../data/index.html"))
}

// Stop or start the simulation but not the program
#[post("/toggleSim")]
async fn toggleSim() -> impl Responder 
{
    let state = simStateManager(false, None);
    match state
    {
        SimulationState::RUNNING => simStateManager(true, Some(SimulationState::STOP)),
        SimulationState::STOP => simStateManager(true, Some(SimulationState::RUNNING)),
        SimulationState::EXIT => SimulationState::EXIT,
        _ => simStateManager(true, Some(SimulationState::STOP))
    };

    HttpResponse::Ok()
}

#[derive(Serialize)]
struct StateQuery
{
    state: String
}

#[get("/simState")]
async fn getSimState() -> ActixResult<impl Responder>
{
    let stateJSON = StateQuery{ state: String::from("running") };
    let state = simStateManager(false, None);
    match state
    {
        SimulationState::RUNNING => stateJSON.state = String::from("running"),
        SimulationState::STOP => stateJSON.state = String::from("stop"),
        SimulationState::PAUSED => stateJSON.state = String::from("paused"),
        _ => stateJSON.state = String::from("error")
    }

    Ok(web::Json(stateJSON));
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


#[derive(Deserialize)]
struct TimerQuery
{
    timer: u64
}

#[post("/setTimer")]
async fn setSimTimer(info: web::Query<TimerQuery>) -> impl Responder
{
    // Converts received time into microseconds, web service expects minutes
    simTimerManager(true, Some(info.timer.clone() as u128 * 1000000 * 60));
    HttpResponse::Ok()
}

// Set up and asynchronously run the Actix HTTP server for the control panel
#[actix_web::main]
pub async fn initWebServer() -> std::io::Result<()>
{
    HttpServer::new(|| {
        App::new()
            .service(getPage)
            .service(toggleSim)
            .service(exitSim)
            .service(suspendSim)
            .service(setSimConfig)
            .service(getSimTime)
            .service(setSimTimer)
            .service(getSimState)
        })
        .disable_signals()
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
