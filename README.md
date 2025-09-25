<div align="center">
<img src="./src/webui/freecaster.svg" alt="Freecaster Logo" width="128" height="128"/>
</div>

# freecaster-grid
Freecaster grid, buddy home lab death notifier
Its purpose is to send telegram notifications in case a home lab in the grid goes down.
To do this, it checks a primary lab, to fetch configuration about other nodes, then checks them.
Once node is detected down, by a mayority of other nodes, nodes agree which one of them is gona send the telegram
message. Then it sends a notif.

freecaster runs a super light weight web server, needs to be mega light weight to ensure we dont take up too many resources.

# TLS

If you have a reverse proxy like nginx or caddy in front of freecaster, you can terminate TLS there.

Freecaster can provide TLS on its own!
To do this with self-signed certs, you can generate a key and cert with openssl:

```
# First generate an EC private key
openssl ecparam -name prime256v1 -genkey -noout -out ./keys/private_key.pem

# Then generate a self-signed certificate using that EC key
openssl req -new -x509 -key ./keys/private_key.pem -out ./keys/certificate.pem -days 365 -subj "/CN=localhost"
openssl pkcs8 -topk8 -nocrypt -in ./keys/private_key.pem -out ./keys/private_key.pkcs.pem
```

Then you can point freecaster to those files in the config:

```yaml
server:
  ssl:
    cert_path: "./keys/certificate.pem"
    key_path: "./keys/private_key.pkcs.pem"
```

When the server.ssl section is present, freecaster will use TLS.
If the cert_path or key_path is missing, the server will refuse to start.

# Usage
Setup a config file for all participating nodes, generate keys, then start the server with
```
cargo run --release -- config.yaml
```

## Docker
There is a dockerized version available for deployment.
```
docker pull ghcr.io/dewyer/freecaster-grid:latest
```
For an example docker compose configuration, see the [compose.yaml file](examples/compose.yaml).

## Configuration

### Configuration via yaml file

This is the default for freecaster-grid. You can specify the config file path as a command line argument.
```bash
freecaster-grid config.yaml
```

Example config:
```yaml
name: hal9000
telegram:
  token: SOME_VERY_LONG_TOKEN
  chat_id: 1234567890 
secret_key: SOME_VERY_LONG_SECRET_KEY # Must be the same on all nodes
webui_enabled: true # Enable web UI at /webui
announcement_mode: telegram # log or telegram
poll_time: 10s # How often to poll other nodes 10s = 10 seconds, 5m = 5 minutes, 1h = 1 hour
server:
  ip_address: "0.0.0.0"
  port: 3037
  ssl:
    cert_path: "./keys/certificate.pem"
    key_path: "./keys/private_key.pkcs.pem"
nodes:
  hal9000: # The key here is not used, it's just for organization
    address: "http://hal9000:3037"
    telegram_handle: hal9000
  hal9001:
    address: "http://hal9001:3037"
    telegram_handle: hal9001
  hal9002:
    address: "http://hal9002:3037"
```

### Configuration via environment variables

You can fully configure freecaster-grid via environment variables as well.
This is especially useful when deploying with docker.

All variables are prefixed with `FC_`, and nested fields are separated by `__` (double underscore).

The above example using environment variables (dotenv format):
```env
FC_NAME=hal9000
FC_TELEGRAM__TOKEN=SOME_VERY_LONG_TOKEN
FC_TELEGRAM__CHAT_ID=1234567890
FC_SECRET_KEY=SOME_VERY_LONG_SECRET_KEY
FC_WEBUI_ENABLED=true
FC_ANNOUNCEMENT_MODE=telegram # log or telegram
FC_POLL_TIME=10s # How often to poll other nodes 10s = 10 seconds, 5m = 5 minutes, 1h = 1 hour
FC_SERVER__IP_ADDRESS=0.0.0.0
FC_SERVER__PORT=3037
FC_SERVER__SSL__CERT_PATH=./keys/certificate.pem
FC_SERVER__SSL__KEY_PATH=./keys/private_key.pkcs.pem
FC_NODES__hal9000__ADDRESS=http://hal9000:3037
FC_NODES__hal9000__TELEGRAM_HANDLE=hal9000
FC_NODES__hal9001__ADDRESS=http://hal9001:3037
FC_NODES__hal9001__TELEGRAM_HANDLE=hal9001
FC_NODES__hal9002__ADDRESS=http://hal9002:3037
```

# Testing
There is a dockerized version available for testing, which enables to run multiple instances of freecaster-grid locally. This can be used to test the application as a whole.
```
docker compose up --build
```

## JSON Schema

The JSON schema for the configuration file is located at `./config.schema.json`.
This can be used to validate your configuration file in your editor, if it supports JSON schema validation.

If you modified the config structure, please also update the schema file.
The schema can be generated with `cargo run -F json_schema -- config.schema.json`.
This will write the schema to the specified file.