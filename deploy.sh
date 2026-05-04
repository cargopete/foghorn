#!/usr/bin/env bash
# Deploy Foghorn to the Hetzner VPS.
# Usage: ./deploy.sh
set -euo pipefail

VPS="root@167.235.29.213"
SSH_KEY="$HOME/.ssh/hetzner_drpc"
REMOTE_DIR="/root/foghorn"

echo "==> Syncing source to VPS..."
rsync -avz --exclude target --exclude .git --exclude node_modules --exclude config.toml \
  -e "ssh -i $SSH_KEY" \
  "$(dirname "$0")/" "$VPS:$REMOTE_DIR/"

echo "==> Creating foghorn database if it doesn't exist..."
ssh -i "$SSH_KEY" "$VPS" \
  "docker exec drpc-postgres-1 psql -U dispatch -tc \
     \"SELECT 1 FROM pg_database WHERE datname='foghorn'\" | grep -q 1 \
   || docker exec drpc-postgres-1 psql -U dispatch -c 'CREATE DATABASE foghorn'"

echo "==> Building and starting Foghorn containers..."
ssh -i "$SSH_KEY" "$VPS" \
  "cd $REMOTE_DIR && docker compose build --no-cache && docker compose up -d"

echo "==> Status:"
ssh -i "$SSH_KEY" "$VPS" \
  "cd $REMOTE_DIR && docker compose ps"

echo ""
echo "Foghorn API is available at http://167.235.29.213:8082/v1/health"
echo "Set FOGHORN_API_URL=http://167.235.29.213:8082 in your Lodestar environment."
