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
  ssl: true
  cert_path: "./keys/certificate.pem"
  key_path: "./keys/private_key.pkcs.pem"
```

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
Example config:
```yaml
name: hal9000
telegram_token: SOME_VERY_LONG_TOKEN
telegram_chat_id: 1234567890
secret_key: SOME_VERY_LONG_SECRET_KEY
webui_enabled: true
server:
  host: "0.0.0.0:3037"
  ssl: false
nodes:
  - name: hal9001
    address: "http://hal9001:3037"
    telegram_handle: hal9001
  - name: hal9002
    address: "http://hal9002:3037"
```

# Testing
There is a dockerized version available for testing, which enables to run multiple instances of freecaster-grid locally. This can be used to test the application as a whole.
```
docker compose up --build
```
