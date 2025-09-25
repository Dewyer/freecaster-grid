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

Freecaster-grid supports two configuration approaches:

### 1. Legacy Combined Configuration (Backward Compatible)

All settings in one file - suitable for simple deployments:

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

### 2. Split Configuration (Recommended)

Separates local instance settings from shared grid configuration:

**Local Config** (`local-config.yaml`):
```yaml
# Personal/instance-specific settings
name: hal9000
server:
  host: "0.0.0.0:3037"
  ssl: false
webui_enabled: true

# Load grid config from file or URL
grid_config_path: "./grid-config.yaml"
# grid_config_url: "https://example.com/grid-config.yaml"
```

**Grid Config** (`grid-config.yaml`):
```yaml
# Shared settings across all nodes
telegram_token: SOME_VERY_LONG_TOKEN
telegram_chat_id: 1234567890
secret_key: SOME_VERY_LONG_SECRET_KEY
poll_time: 30s
announcement_mode: telegram

nodes:
  - name: hal9000
    address: "http://hal9000:3037"
    telegram_handle: hal9000_user
  - name: hal9001
    address: "http://hal9001:3037"
    telegram_handle: hal9001_user
```

### Environment Variable Overrides

All configuration values can be overridden using environment variables with the `FREECASTER_` prefix:

```bash
# Basic settings
FREECASTER_NAME=my-node
FREECASTER_TELEGRAM_TOKEN=your-token-here
FREECASTER_TELEGRAM_CHAT_ID=123456789
FREECASTER_SECRET_KEY=your-secret-key
FREECASTER_POLL_TIME=60s
FREECASTER_ANNOUNCEMENT_MODE=log
FREECASTER_WEBUI_ENABLED=true

# Server settings
FREECASTER_SERVER_HOST=0.0.0.0:8080
FREECASTER_SERVER_SSL=true
FREECASTER_SERVER_CERT_PATH=./certs/cert.pem
FREECASTER_SERVER_KEY_PATH=./certs/key.pem
```

### Security Best Practices

For production deployments:

1. **Use environment variables for secrets** instead of putting them in config files:
   ```bash
   FREECASTER_TELEGRAM_TOKEN=your-actual-token
   FREECASTER_SECRET_KEY=your-actual-secret
   ```

2. **Use centralized grid configuration** for easier maintenance:
   ```yaml
   # local-config.yaml
   grid_config_url: "https://your-private-repo.com/grid-config.yaml"
   auto_update_grid_config: true
   ```

3. **Store sensitive configs separately** from your main configuration files

See the [examples directory](examples/) for complete configuration examples.

# Testing
There is a dockerized version available for testing, which enables to run multiple instances of freecaster-grid locally. This can be used to test the application as a whole.
```
docker compose up --build
```
