# homie-telegraf
Telegraf input plugin (via socket) for homie messages

This plugin is meant to be used for any MQTT service that implements the *homie-v4* protocol for messages being sent from an MQTT server. The app is 
wrapped in a `Docker` setup so running it is quite simple:

- Configure your telegraf server to inject a socket as follows:

```
[[inputs.socket_listener]]
  service_address = "tcp://0.0.0.0:5094"

```

- Next make sure you've added an output processors to the service you'd like to send to - most likely `Influxdb`
```
[[outputs.influxdb_v2]]
    urls = ["http://192.168.1.158:8086"]
    token = "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX=="
    organization = "XXXXXXXXX"
    bucket = "HVAC-GEO"

```

- Edit the `docker-compose.yml` file to modify the ports, addresses, usernames, and passwords for your environment

- Next use `docker-compose create` to build the Docker image and container

And that's it

# Next Steps
Homie messages can contain non-numeric fields. I think I am going to implement a mapping yaml file that lets you specify how to map non-numeric items. For
example, `standby`, `low`, `high` might be an enum coming from a topic. Mapping these to `0`, `1`, and, `2` might be away to see levels and changes between them


