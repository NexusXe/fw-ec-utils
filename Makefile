# Get the UID of the user running the command (or the original user if using sudo)
SUDO_USER ?= $(USER)
UID := $(shell id -u $(SUDO_USER))

BIN_NAME = fw-fanctrl-rs
SERVICE_NAME = fw-fanctrl@.service

BIN_DIR = /usr/local/bin
SERVICE_DIR = /etc/systemd/system

CURVES_DIR = /etc/$(BIN_NAME)/curves
CONFIG_DIR = /etc/$(BIN_NAME)
LICENSE_DIR = /usr/share/licenses/$(BIN_NAME)

.PHONY: all build install uninstall restart status clean

all: build

build:
	cargo build --release

install:
	@echo "Installing $(BIN_NAME) to $(BIN_DIR)..."
	install -Dm755 target/release/$(BIN_NAME) $(BIN_DIR)/$(BIN_NAME)
	@echo "Installing systemd service..."
	install -Dm644 fw-fanctrl.service $(SERVICE_DIR)/$(SERVICE_NAME)
	@echo "Installing default external curves to $(CURVES_DIR)..."
	install -d $(CURVES_DIR)
	install -Dm644 curves/* $(CURVES_DIR)/
	@echo "Installing default config to $(CONFIG_DIR)..."
	install -Dm644 config.toml $(CONFIG_DIR)/config.toml
	@echo "Installing license..."
	install -Dm644 LICENSE $(LICENSE_DIR)/LICENSE
	systemctl daemon-reload
	@echo "Install complete."
	@echo "To start and enable the fan controller, run:"
	@echo "   sudo systemctl enable --now fw-fanctrl@\$$(id -u).service"

uninstall:
	@echo "Stopping active fw-fanctrl services..."
	systemctl stop 'fw-fanctrl@*.service' > /dev/null 2>&1 || true
	@echo "Removing files..."
	rm -f $(BIN_DIR)/$(BIN_NAME)
	rm -f $(SERVICE_DIR)/$(SERVICE_NAME)
	rm -rf $(CURVES_DIR)
	rm -f $(CONFIG_DIR)/config.toml
	rm -rf $(LICENSE_DIR)
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