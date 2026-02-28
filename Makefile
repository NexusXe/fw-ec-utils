# Get the UID of the user running the command (or the original user if using sudo)
SUDO_USER ?= $(USER)
UID := $(shell id -u $(SUDO_USER))

BIN_NAME = fw-fanctrl-rs
SERVICE_NAME = fw-fanctrl@.service

BIN_DIR = /usr/local/bin
SERVICE_DIR = /etc/systemd/system

CURVES_DIR = /etc/$(BIN_NAME)/curves

.PHONY: all build install uninstall restart status clean

all: build

build:
	cargo build --release

install:
	@echo "Installing $(BIN_NAME) to $(BIN_DIR)..."
	install -m 755 target/release/$(BIN_NAME) $(BIN_DIR)/$(BIN_NAME)
	@echo "Installing default external curves to $(CURVES_DIR)..."
	install -d $(CURVES_DIR)
	install -m 644 curves/* $(CURVES_DIR)/
	@echo "Installing systemd service..."
	install -m 644 fw-fanctrl.service $(SERVICE_DIR)/$(SERVICE_NAME)
	systemctl daemon-reload
	systemctl enable fw-fanctrl@$(UID).service
	systemctl restart fw-fanctrl@$(UID).service
	@echo "Install complete. Daemon is running."

uninstall:
	@echo "Stopping and disabling service..."
	systemctl stop fw-fanctrl@$(UID).service || true
	systemctl disable fw-fanctrl@$(UID).service || true
	@echo "Removing files..."
	rm -f $(BIN_DIR)/$(BIN_NAME)
	rm -f $(SERVICE_DIR)/$(SERVICE_NAME)
	rm -rf $(CURVES_DIR)
	systemctl daemon-reload
	@echo "Uninstall complete."

restart:
	systemctl restart fw-fanctrl@$(UID).service

status:
	systemctl status fw-fanctrl@$(UID).service

logs:
	journalctl -u fw-fanctrl@$(UID).service -f

clean:
	cargo clean