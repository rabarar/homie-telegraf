//! Example to discover all Homie devices, and log whenever a property value changes.

use homie_controller::{Event, HomieController, PollError};
use rumqttc::MqttOptions;
use std::time::Duration;
use telegraf::*;

#[derive(Debug)]
#[derive(Metric)]
struct MyMetric {
    value: f32,
    #[telegraf(tag)]
    device_id_tag: String,
    #[telegraf(tag)]
    node_id_tag: String,
    #[telegraf(tag)]
    property_id_tag: String,

}

#[tokio::main]
async fn main() -> Result<(), PollError> {
    pretty_env_logger::init();

    let mut client = Client::new("tcp://192.168.1.158:5094").unwrap();

    let mut mqttoptions = MqttOptions::new("homie_controller", "192.168.1.158", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials("admin", "password");

    let (controller, mut event_loop) = HomieController::new(mqttoptions, "homie");
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
                        println!(
                            "-> {}/{}/{} = {} ({})",
                            device_id, node_id, property_id, value, fresh
                        );

                        let point = MyMetric {
                            value: match value.parse() {
                                Ok(val) => {
                                    val
                                }
                                Err(_e) => {
                                    println!("can't convert {} to float, setting to 0.0", value);
                                    0.0

                                }
                            },
                            device_id_tag: device_id,
                            node_id_tag: node_id,
                            property_id_tag: property_id,
                        };

                        match client.write(&point) {
                            Ok(val) => {
                            }
                            Err(e) => {
                                println!("error writing: {}", e);
                            }
                        }
                        //println!("writing {:#?}", &point);
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
            Err(e) => log::error!("Error: {:?}", e),
        }
    }
}
