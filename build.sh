#!/bin/dash

cargo build --release

sudo systemctl stop fw-chargemon.service

sudo cp target/release/fw-chargemon /usr/local/bin/
sudo cp target/release/fw-chargemon-query /usr/local/bin/

sudo systemctl try-restart fw-chargemon.service
