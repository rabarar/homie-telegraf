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
                                    error!("can't convert {} to float, setting to 0.0", value);
                                    0.0

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
