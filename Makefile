XDG_CONFIG_HOME ?= $(HOME)/.config
XDG_DATA_HOME ?= $(HOME)/.local/share
CONFIG_DIR ?= $(XDG_CONFIG_HOME)/streamrs
PROFILE ?= default
PROFILE_DIR ?= $(XDG_DATA_HOME)/streamrs/$(PROFILE)
BIN_DIR ?= $(HOME)/.local/bin
BIN_NAME ?= streamrs
SYSTEMD_USER_DIR ?= $(XDG_CONFIG_HOME)/systemd/user
SERVICE_NAME ?= streamrs
SERVICE_FILE ?= $(SYSTEMD_USER_DIR)/$(SERVICE_NAME).service
SERVICE_TEMPLATE ?= systemd/streamrs.service

.PHONY: build install-bin install-systemd install install-config install-images install-assets
.PHONY: uninstall-bin uninstall-systemd uninstall-config uninstall-images uninstall-assets uninstall

build:
	cargo build --release

install-bin: build
	mkdir -p "$(BIN_DIR)"
	install -m 0755 "target/release/$(BIN_NAME)" "$(BIN_DIR)/$(BIN_NAME)"

install-config:
	mkdir -p "$(CONFIG_DIR)"
	cp "config/default.toml" "$(CONFIG_DIR)/default.toml"

install-images:
	mkdir -p "$(PROFILE_DIR)"
	cp -a all_images/. "$(PROFILE_DIR)/"

install-assets: install-config install-images

install-systemd: install-bin
	mkdir -p "$(SYSTEMD_USER_DIR)"
	install -m 0644 "$(SERVICE_TEMPLATE)" "$(SERVICE_FILE)"
	systemctl --user daemon-reload
	systemctl --user enable "$(SERVICE_NAME).service"
	if systemctl --user is-active --quiet "$(SERVICE_NAME).service"; then \
		systemctl --user restart "$(SERVICE_NAME).service"; \
	else \
		systemctl --user start "$(SERVICE_NAME).service"; \
	fi

install: install-bin install-assets install-systemd

uninstall-bin:
	rm -f "$(BIN_DIR)/$(BIN_NAME)"

uninstall-systemd:
	-systemctl --user disable --now "$(SERVICE_NAME).service"
	rm -f "$(SERVICE_FILE)"
	-systemctl --user daemon-reload

uninstall-config:
	rm -f "$(CONFIG_DIR)/$(PROFILE).toml"

uninstall-images:
	rm -rf "$(PROFILE_DIR)"

uninstall-assets: uninstall-config uninstall-images

uninstall: uninstall-systemd uninstall-assets uninstall-bin
