#!/bin/dash

cargo build --release

# The stop is load-bearing: writing over a running executable fails with ETXTBSY.
sudo systemctl stop fw-chargemon.service

sudo cp target/release/fw-chargemon /usr/local/bin/
sudo cp target/release/fw-chargemon-query /usr/local/bin/

# `start`, not `try-restart` — try-restart is deliberately a no-op on a unit
# that is not running, so pairing it with the stop above left the service down.
sudo systemctl start fw-chargemon.service
