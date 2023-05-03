use std::fmt;
use std::process;
use std::str::FromStr;

use clap::Parser;

use homie_controller::{Event, HomieController, PollError};
use rumqttc::MqttOptions;
use std::time::Duration;

use telegraf::*;

extern crate influxdb_rs;
use chrono::prelude::*;
use url::Url;

use env_logger::Env;

use serde::Deserialize;

#[macro_use]
extern crate log;

#[derive(Debug, Metric)]
struct HomieMetric {
    value: f32,
    #[telegraf(tag)]
    device_id_tag: String,
    #[telegraf(tag)]
    node_id_tag: String,
    #[telegraf(tag)]
    property_id_tag: String,
}

fn in_zone_priority(s: &str) -> bool {
    match s {
        "economy" | "comfort" => true,
        _ => false,
    }
}

fn in_current_mode(s: &str) -> bool {
    match s {
        "lockout" | "standby" | "blower" | "heating" | "heating_with_aux" | "emergency_heat"
        | "cooling" | "waiting" | "h1" | "h2" | "h3" | "c1" | "c2" => true,
        _ => false,
    }
}

fn in_target_fan_mode(s: &str) -> bool {
    match s {
        "auto" | "continuous" | "intermittent" => true,
        _ => false,
    }
}

fn in_target_mode(s: &str) -> bool {
    match s {
        "off" | "auto" | "cool" | "heat" | "eheat" => true,
        _ => false,
    }
}

fn in_humidifier_mode(s: &str) -> bool {
    match s {
        "auto" | "manual" => true,
        _ => false,
    }
}

fn current_mode_to_value(s: &str) -> Option<f32> {
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
            _ => 0f32,
        })
    }
}

fn humidifier_mode_to_value(s: &str) -> Option<f32> {
    if in_humidifier_mode(s) != true {
        None
    } else {
        Some(match s {
            "auto" => 1f32,
            "manual" => 2f32,
            _ => 0f32,
        })
    }
}

fn zone_priority_to_value(s: &str) -> Option<f32> {
    if in_zone_priority(s) != true {
        None
    } else {
        Some(match s {
            "economy" => 1f32,
            "comfort" => 2f32,
            _ => 0f32,
        })
    }
}

fn target_mode_to_value(s: &str) -> Option<f32> {
    if in_target_mode(s) != true {
        None
    } else {
        Some(match s {
            "off" => 1f32,
            "auto" => 2f32,
            "cool" => 3f32,
            "heat" => 4f32,
            "eheat" => 5f32,
            _ => 0f32,
        })
    }
}

fn target_fan_mode_to_value(s: &str) -> Option<f32> {
    if in_target_fan_mode(s) != true {
        None
    } else {
        Some(match s {
            "auto" => 1f32,
            "continuous" => 2f32,
            "intermittent " => 3f32,
            _ => 0f32,
        })
    }
}

const TELEGRAF_HOST: &str = "192.168.1.158";
const TELEGRAF_INPUT_SOCKET: u16 = 5094;

const INFLUX_HOST: &str = "192.168.1.158";
const INFLUX_PORT: u16 = 8086;
const INFLUX_BUCKET: &str = "HVAC-GEO";
const INFLUX_ORG: &str = "Baruch";

const MQTT_HOST: &str = "192.168.1.158";
const MQTT_PORT: u16 = 1883;
const HOMIE_TOPIC: &str = "homie";

#[allow(dead_code)]
#[derive(Debug)]
enum TelTransport {
    Udp,
    Tcp,
}

impl fmt::Display for TelTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelTransport::Udp => write!(f, "udp"),
            TelTransport::Tcp => write!(f, "tcp"),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
enum PushMethod {
    Influx,
    Telegraf,
}

impl fmt::Display for PushMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushMethod::Influx => write!(f, "influx"),
            PushMethod::Telegraf => write!(f, "telegraf"),
        }
    }
}

impl FromStr for PushMethod {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "influx" => Ok(PushMethod::Influx),
            "telegraf" => Ok(PushMethod::Telegraf),
            _ => Err(()),
        }
    }
}

#[derive(Parser, Debug)]
//#[command(author, version, about, long_about = None)]
struct Args {
    ///  Push method: Telegraf or influx
    #[arg(short='x', long, default_value_t = PushMethod::Telegraf.to_string())]
    push_method: String,

    /// telegraf hostname for homie socket input processor
    #[arg(short, long, default_value_t = TELEGRAF_HOST.to_string())]
    tel_host: String,

    /// telegraf port for homie socket input processor (5094)
    #[arg(short='p', long, default_value_t = TELEGRAF_INPUT_SOCKET)]
    tel_port: u16,

    /// telegraf transport: Udp or Tcp
    #[arg(short='r', long, default_value_t = TelTransport::Udp.to_string())]
    tel_transport: String,

    /// MQTT hostname
    #[arg(short, long, default_value_t = MQTT_HOST.to_string())]
    mqtt_host: String,

    /// MQTT port (1883)
    #[arg(short='q', long, default_value_t = MQTT_PORT)]
    mqtt_port: u16, // 1883

    /// MQTT topic (homie)
    #[arg(short='o', long, default_value_t = HOMIE_TOPIC.to_string())]
    mqtt_topic: String, // homie

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,

    /// Influx Hostname
    #[arg(short='f', long, default_value_t = INFLUX_HOST.to_string())]
    influx_host: String,

    /// Influx port (8086)
    #[arg(short='i', long, default_value_t = INFLUX_PORT)]
    influx_port: u16,

    /// Influx Bucket
    #[arg(short='b', long, default_value_t = INFLUX_BUCKET.to_string())]
    influx_bucket: String,

    /// Influx Org
    #[arg(short='g', long, default_value_t = INFLUX_ORG.to_string())]
    influx_org: String,
}

#[derive(Deserialize, Debug)]
struct EnvConfig {
    mqtt_username: String, // admin
    mqtt_password: String, // password
    influx_key: String,    // see influx
}

#[tokio::main]
async fn main() -> Result<(), PollError> {
    // setup logging
    let env = Env::default()
        .filter_or("HOMIEGRAF_LEVEL", "trace")
        .write_style_or("HOMIEGRAF_STYLE", "always");

    env_logger::init_from_env(env);

    // see if the config is setup
    let env_config = envy::prefixed("HOMIE_")
        .from_env::<EnvConfig>()
        .expect("missing environmental variables");

    // setup command-line processing
    let cli = Args::parse();

    let push_method = PushMethod::from_str(&cli.push_method).map_err(|e| {
        error!(
            "{}",
            &format!(
                "Invalid push method specified: {}, {:?}",
                cli.push_method, e
            )
        );
        process::exit(1);
    });

    info!("using push method [{:?}]", push_method.as_ref());

    if !cli.tel_host.is_empty() {
        info!("using telegraf host: [{}]", cli.tel_host)
    } else {
        error!("no telegraf host specified, exiting.");
        process::exit(1);
    }

    info!("using telegraf port: [{:?}]", cli.tel_port);

    let mut telegraf_client = Client::new(&format!(
        "{}://{}:{}",
        TelTransport::Udp,
        cli.tel_host,
        cli.tel_port
    ))
    .expect(&format!(
        "failed to connect to {}:{}",
        cli.tel_host, cli.tel_port
    ));

    info!(
        "using influx: {}:{} Bucket=[{}], Org=[{}]",
        cli.influx_host, cli.influx_port, cli.influx_bucket, cli.influx_org
    );
    let influx_client = influxdb_rs::Client::new(
        Url::parse(&format!("http://{}:{}", cli.influx_host, cli.influx_port)).unwrap(),
        cli.influx_bucket,
        cli.influx_org,
        env_config.influx_key,
    )
    .await
    .unwrap();

    if !cli.mqtt_host.is_empty() {
        info!("using MQTT host: [{}]", cli.mqtt_host)
    } else {
        error!("no MQTT host specified, exiting.");
        process::exit(1);
    }

    info!("using MQTT port: [{}]", cli.mqtt_port);

    trace!(
        "using MQTT Creds: [{}, {}]",
        env_config.mqtt_username,
        env_config.mqtt_password
    );
    trace!("using MQTT topic: [{}]", cli.mqtt_topic);

    let mut mqttoptions = MqttOptions::new(
        &format!("homie_controller_{}", process::id()),
        cli.mqtt_host,
        cli.mqtt_port,
    );

    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials(env_config.mqtt_username, env_config.mqtt_password);

    // set the topic - likely should be homie
    if cli.mqtt_topic.is_empty() {
        error!("no MQTT topic was specified, exiting.");
        process::exit(1);
    }

    let (controller, mut event_loop) = HomieController::new(mqttoptions, &cli.mqtt_topic);

    loop {
        match controller.poll(&mut event_loop).await {
            Ok(events) => {
                for event in events {
                    if let Event::PropertyValueChanged {
                        device_id,
                        node_id,
                        property_id,
                        value,
                        fresh: _,
                    } = event
                    {
                        // trace!( "{}/{}/{} = {} ({})", device_id, node_id, property_id, value, fresh);

                        let point = HomieMetric {
                            value: match value.parse() {
                                Ok(val) => val,
                                Err(_e) => {
                                    // for obvious values, let's convert to a numeric value
                                    match value.as_str() {
                                        "true" | "open" => 1.0,
                                        "false" | "closed" => 0.0,
                                        s if in_current_mode(s) => {
                                            current_mode_to_value(s).unwrap()
                                        }
                                        s if in_humidifier_mode(s) => {
                                            humidifier_mode_to_value(s).unwrap()
                                        }
                                        s if in_target_mode(s) => target_mode_to_value(s).unwrap(),
                                        s if in_target_fan_mode(s) => {
                                            target_fan_mode_to_value(s).unwrap()
                                        }
                                        s if in_zone_priority(s) => {
                                            zone_priority_to_value(s).unwrap()
                                        }
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

                        //if PushMethod::from_str(&cli.push_method).unwrap() == PushMethod::Telegraf {
                        if push_method == Ok(PushMethod::Telegraf) {
                            match telegraf_client.write(&point) {
                                Ok(_val) => {
                                    trace!("writing point: {:?}", &point);
                                }
                                Err(e) => {
                                    error!("failed to write point, error writing: {}", e);
                                    let retry = false;
                                    if retry {
                                        info!("attempting to reconnect");
                                        drop(telegraf_client);
                                        telegraf_client = Client::new(&format!(
                                            "tcp://{}:{}",
                                            cli.tel_host, cli.tel_port
                                        ))
                                        .expect(&format!(
                                            "failed to connect to {}:{}",
                                            cli.tel_host, cli.tel_port
                                        ));
                                        info!("reconnected, attempting to write point...");
                                        match telegraf_client.write(&point) {
                                            Ok(_) => {
                                                trace!(
                                                    "successfully reconnected and wrote point {:?}",
                                                    &point
                                                );
                                            }
                                            Err(e) => {
                                                error!("failed to write point after attempted reconnect: {}", e);
                                                panic!("terminal error, cannot reconnect to telegraf server");
                                            }
                                        }
                                    } else {
                                        panic!("terminating...");
                                    }
                                }
                            }
                        } else {
                            let now = Utc::now();
                            let influx_point = influxdb_rs::Point::new("HomieMetric")
                                .add_tag("device_id_tag", point.device_id_tag)
                                .add_tag("node_id_tag", point.node_id_tag)
                                .add_tag("property_id_tag", point.property_id_tag)
                                .add_field("value", point.value)
                                .add_timestamp(now.timestamp());

                            info!("influx: attempting to write point: [{:?}]", &influx_point);
                            let res = influx_client
                                .write_point(
                                    influx_point,
                                    Some(influxdb_rs::Precision::Seconds),
                                    None,
                                )
                                .await;
                            match res {
                                Ok(_) => {
                                    info!("influxdb: wrote point to influx db");
                                }
                                Err(e) => {
                                    error!("influxdb: failed to write point to influx db: {}", e);
                                }
                            }
                        }
                    } else {
                        //println!("Event: {}/{}/{}", event.device_id, event.node_id, event.propert_id);
                        //println!("Devices:");
                        for device in controller.devices().values() {
                            if device.has_required_attributes() {
                                info!(" * {}", device.id);
                            } else {
                                info!(" * {} not ready.", device.id);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Homie Controller Poll Error: {:?}", e);
                process::exit(1);
            }
        }
    }
}
