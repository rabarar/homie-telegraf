use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

use homie_controller::{Event, HomieController, PollError};
use rumqttc::MqttOptions;
use std::time::Duration;
use telegraf::*;
use env_logger::{Env};

use serde::Deserialize;

#[macro_use]
extern crate log;

#[derive(Debug)]
#[derive(Metric)]
struct HomieMetric {
    value: f32,
    #[telegraf(tag)]
    device_id_tag: String,
    #[telegraf(tag)]
    node_id_tag: String,
    #[telegraf(tag)]
    property_id_tag: String,

}

fn in_zone_priority(s:&str) -> bool {
    match s {
        "economy" | "comfort"  => true,
        _ => false
    }
}

fn in_current_mode(s:&str) -> bool {
    match s {
        "lockout" | "standby" |  "blower" | "heating"
            | "heating_with_aux" | "emergency_heat"
            | "cooling" | "waiting" | "h1" | "h2" | "h3"| "c1" | "c2" => true,
        _ => false
    }
}

fn in_target_fan_mode(s:&str) -> bool {
    match s {
        "auto" | "continuous" | "intermittent" => true,
        _ => false
    }
}

fn in_target_mode(s:&str) -> bool {
    match s {
        "off" | "auto" | "cool" | "heat" | "eheat" => true,
        _ => false
    }
}

fn in_humidifier_mode(s:&str) -> bool {
    match s {
        "auto" | "manual" => true,
        _ => false
    }
}

fn current_mode_to_value(s:&str) -> Option<f32> {

    if in_current_mode(s) != true {
        None
    } else {
        Some(match s {
            "lockout" => 1f32,
            "standby" => 2f32,
            "blower" => 3f32,
            "heating" => 4f32,
            "heating_with_aux" => 5f32,
            "emergency_heat" => 6f32,
            "cooling" => 7f32,
            "waiting" => 8f32,
            "h1" => 2.1,
            "h2" => 2.2,
            "h3" => 2.3,
            "c1" => 2.4,
            "c2" => 2.5,
            _ => 0f32
        })

    }
}

fn humidifier_mode_to_value(s:&str) -> Option<f32> {

    if in_humidifier_mode(s) != true {
        None
    } else {
        Some(match s {
            "auto" => 1f32,
            "manual" => 2f32,
            _ => 0f32
        })

    }
}

fn zone_priority_to_value(s:&str) -> Option<f32> {

    if in_zone_priority(s) != true {
        None
    } else {
        Some(match s {
            "economy" => 1f32,
            "comfort" => 2f32,
            _ => 0f32
        })

    }
}

fn target_mode_to_value(s:&str) -> Option<f32> {

    if in_target_mode(s) != true {
        None
    } else {
        Some(match s {
            "off" => 1f32,
            "auto" => 2f32,
            "cool" => 3f32,
            "heat" => 4f32,
            "eheat" => 5f32,
            _ => 0f32
        })

    }
}

fn target_fan_mode_to_value(s:&str) -> Option<f32> {

    if in_target_fan_mode(s) != true {
        None
    } else {
        Some(match s {
            "auto" => 1f32,
            "continuous" => 2f32,
            "intermittent " => 3f32,
            _ => 0f32
        })

    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// telegraf hostname for homie socket input processor
    tel_host: Option<String>,
    /// telegraf port for homie socket input processor (5094)
    tel_port: Option<u16>, // 5094

    /// MQTT hostname
    mqtt_host: Option<String>,
    /// MQTT port (1883)
    mqtt_port: Option<u16>, // 1883
    /// MQTT topic (homie)
    mqtt_topic: Option<String>, // homie

    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// does testing things
    Test {
        /// lists test values
        #[arg(short, long)]
        list: bool,
    },
}

#[derive(Deserialize, Debug)]
struct MQTTCreds {
    username: String, // admin
    password: String, // password
}

#[tokio::main]
async fn main() -> Result<(), PollError> {

    // setup logging
    let env = Env::default()
        .filter_or("HOMIEGRAF_LEVEL", "trace")
        .write_style_or("HOMIEGRAF_STYLE", "always");

    env_logger::init_from_env(env);


    // setup command-line processing
    let cli = Cli::parse();

    let mut host:&str;
    let mut port:u16;

    if let Some(h) = cli.tel_host.as_deref() {
        host = h;
        info!("using telegraf host: [{}]", host)
    } else {
        error!("no telegraf host specified, exiting.");
        process::exit(1);
    }


    if let Some(p) = cli.tel_port {
        port = p;
        info!("using telegraf port: [{}]", port)
    } else {
        error!("no telegraf port specified, exiting.");
        process::exit(1);
    }

    let mut telegraf_client = Client::new(&format!("tcp://{}:{}", host, port))
        .expect(&format!("failed to connect to {}:{}", host, port));


    if let Some(h) = cli.mqtt_host.as_deref() {
        host = h;
        info!("using MQTT host: [{}]", host)
    } else {
        error!("no MQTT host specified, exiting.");
        process::exit(1);
    }

    if let Some(p) = cli.mqtt_port {
        port = p;
        info!("using MQTT port: [{}]", port)
    } else {
        error!("no MQTT port specified, exiting.");
        process::exit(1);
    }

    // see if the username and password are set
    let mqtt_creds = envy::prefixed("MQTT_")
        .from_env::<MQTTCreds>()
        .expect("Please provide MQTT_USERNAME and MQTT_PASSWORD env vars");

    trace!("using MQTT Creds: [{:#?}]", mqtt_creds);

    let mut mqttoptions = MqttOptions::new("homie_controller", host, port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials(mqtt_creds.username, mqtt_creds.password);

    // set the topic - likely should be homie
    let topic:&str;
    if let Some(t) = cli.mqtt_topic.as_deref() {
        topic = t;
    } else {
        error!("no MQTT topic was specified, exiting.");
        process::exit(1);
    }

    let (controller, mut event_loop) = HomieController::new(mqttoptions, topic);

    loop {
        match controller.poll(&mut event_loop).await {
            Ok(events) => {
                for event in events {
                    if let Event::PropertyValueChanged {
                        device_id,
                        node_id,
                        property_id,
                        value,
                        fresh,
                    } = event
                    {
                        trace!(
                            "{}/{}/{} = {} ({})",
                            device_id, node_id, property_id, value, fresh
                        );

                        let point = HomieMetric {
                            value: match value.parse() {
                                Ok(val) => {
                                    val
                                }
                                Err(_e) => {
                                    // for obvious values, let's convert to a numeric value
                                    match value.as_str() { 
                                        "true" | "open"  => 1.0,
                                        "false" | "closed"  => 0.0,
                                        s if in_current_mode(s) =>  current_mode_to_value(s).unwrap(),
                                        s if in_humidifier_mode(s) => humidifier_mode_to_value(s).unwrap(),
                                        s if in_target_mode(s) => target_mode_to_value(s).unwrap(),
                                        s if in_target_fan_mode(s) => target_fan_mode_to_value(s).unwrap(),
                                        s if in_zone_priority(s) => zone_priority_to_value(s).unwrap(),
                                        _ => {
                                            error!("can't convert {} to float for {}/{}/{}, setting to 0.0", value, device_id, node_id, property_id);
                                            0.0
                                        }
                                    }

                                }
                            },
                            device_id_tag: device_id,
                            node_id_tag: node_id,
                            property_id_tag: property_id,
                        };

                        match telegraf_client.write(&point) {
                            Ok(_val) => {
                                trace!("writing point: {:?}", &point);
                            }
                            Err(e) => {
                                error!("failed to write point, error writing: {}", e);
                                info!("attempting to reconnect");
                                drop(telegraf_client);
                                telegraf_client = Client::new(&format!("tcp://{}:{}", host, port))
                                    .expect(&format!("failed to connect to {}:{}", host, port));
                                match telegraf_client.write(&point) {
                                    Ok(_) => {
                                        trace!("successfully reconnected and wrote point {:?}", &point);
                                    }
                                    Err(e) => {
                                        error!("failed to write point after attempted reconnect: {}", e);
                                        panic!("terminal error, cannot reconnect to telegraf server");
                                    }
                                }

                            }
                        }
                    } else {
                        //println!("Event: {:?}", event);
                        //println!("Devices:");
                        for device in controller.devices().values() {
                            if device.has_required_attributes() {
                                //println!(" * {:?}", device);
                            } else {
                                //println!(" * {} not ready.", device.id);
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Error: {:?}", e),
        }
    }
}
