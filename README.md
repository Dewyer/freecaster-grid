# freecaster-grid
Freecaster grid, buddy home lab death notifier
Its purpose is to send telegram notifications in case a home lab in the grid goes down.
To do this, it checks a primary lab, to fetch configuration about other nodes, then checks them.
Once node is detected down, by a mayority of other nodes, nodes agree which one of them is gona send the telegram
message. Then it sends a notif.

freecaster runs a super light weight web server, needs to be mega light weight to ensure we dont take up too many resources.

# Key stuff
```
# First generate an EC private key
openssl ecparam -name prime256v1 -genkey -noout -out ./keys/private_key.pem

# Then generate a self-signed certificate using that EC key
openssl req -new -x509 -key ./keys/private_key.pem -out ./keys/certificate.pem -days 365 -subj "/CN=localhost"
openssl pkcs8 -topk8 -nocrypt -in ./keys/private_key.pem -out ./keys/private_key.pkcs.pem
```

# Usage
Setup a config file for all participating nodes, generate keys, then start the server with
```
cargo run --release -- config.yaml
```

# Docker
There is a dockerized version available for testing, which enables to run multiple instances of freecaster-grid.
```
docker compose up --build
```
