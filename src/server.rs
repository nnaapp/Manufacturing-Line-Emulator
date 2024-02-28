extern crate local_ip_address;
use self::local_ip_address::local_ip;

extern crate opcua;
use opcua::server::prelude::*;

extern crate log2;
use log2::*;

use std::path::PathBuf;

pub fn initServer() -> Server
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
