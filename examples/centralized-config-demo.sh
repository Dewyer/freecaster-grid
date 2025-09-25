#!/bin/bash

# Example script showing how to set up centralized grid configuration
# This demonstrates the "Improvement potential 2" from the issue

echo "=== Freecaster Grid Centralized Configuration Demo ==="

# Create a temporary directory for our demo
DEMO_DIR="/tmp/freecaster-grid-demo"
mkdir -p "$DEMO_DIR"
cd "$DEMO_DIR"

echo "1. Setting up centralized grid configuration..."

# Create a mock centralized grid config
cat > centralized-grid.yaml << 'EOF'
telegram_token: "DEMO_TOKEN_SET_VIA_ENV_VAR"
telegram_chat_id: 123456789
secret_key: "DEMO_SECRET_SET_VIA_ENV_VAR"
poll_time: 30s
announcement_mode: log

nodes:
  - name: node1
    address: "http://node1.example.com:3037"
    telegram_handle: "admin1"
  - name: node2
    address: "http://node2.example.com:3037"
    telegram_handle: "admin2"
  - name: node3
    address: "http://node3.example.com:3037"
    telegram_handle: "admin3"
EOF

echo "✓ Created centralized grid config"

# Create multiple node local configs that reference the centralized grid config
for i in {1..3}; do
    cat > "node${i}-local.yaml" << EOF
name: node${i}
server:
  host: "0.0.0.0:303${i}"
  ssl: false
webui_enabled: true

# Reference centralized grid config
grid_config_path: "./centralized-grid.yaml"
# In production, this would be a URL like:
# grid_config_url: "https://git.example.com/raw/main/grid-config.yaml"
EOF
    echo "✓ Created local config for node${i}"
done

echo ""
echo "2. Demonstrating environment variable overrides..."

# Show how to override sensitive values via environment variables
cat > run-node.sh << 'EOF'
#!/bin/bash

# Set sensitive values via environment variables (best practice)
export FREECASTER_TELEGRAM_TOKEN="real-telegram-token-here"
export FREECASTER_TELEGRAM_CHAT_ID="987654321"
export FREECASTER_SECRET_KEY="real-secret-key-here"

# Override node-specific settings
export FREECASTER_NAME="${1:-node1}"
export FREECASTER_SERVER_HOST="0.0.0.0:${2:-3037}"

echo "Starting $FREECASTER_NAME on $FREECASTER_SERVER_HOST"
echo "Using environment variables for sensitive data"

# In real deployment, this would be:
# freecaster-grid node1-local.yaml
EOF

chmod +x run-node.sh
echo "✓ Created example run script with environment variables"

echo ""
echo "3. File structure created:"
tree . 2>/dev/null || ls -la

echo ""
echo "=== Benefits of this approach ==="
echo "✓ Single source of truth for grid configuration"
echo "✓ Easy to add/remove nodes across the entire grid"
echo "✓ Centralized configuration can be version controlled"
echo "✓ Sensitive data kept separate via environment variables"
echo "✓ Each node only needs minimal local configuration"

echo ""
echo "=== Production deployment example ==="
echo "1. Host grid-config.yaml in private Git repository"
echo "2. Each node references: grid_config_url: 'https://git.example.com/raw/main/grid-config.yaml'"
echo "3. Use auto_update_grid_config: true for automatic updates"
echo "4. Set secrets via environment variables or secret management system"

echo ""
echo "Demo files created in: $DEMO_DIR"