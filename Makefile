XDG_CONFIG_HOME ?= $(HOME)/.config
XDG_DATA_HOME ?= $(HOME)/.local/share
CONFIG_DIR ?= $(XDG_CONFIG_HOME)/streamrs
PROFILE ?= default
PROFILE_DIR ?= $(XDG_DATA_HOME)/streamrs/$(PROFILE)
BIN_DIR ?= $(HOME)/.local/bin
BIN_NAME ?= streamrs
PREVIEW_BIN_NAME ?= streamrs-preview
GUI_BIN_NAME ?= streamrs-gui
ICON_COMPOSE_BIN_NAME ?= streamrs-icon-compose
SYSTEMD_USER_DIR ?= $(XDG_CONFIG_HOME)/systemd/user
SERVICE_NAME ?= streamrs
SERVICE_FILE ?= $(SYSTEMD_USER_DIR)/$(SERVICE_NAME).service
SERVICE_TEMPLATE ?= systemd/streamrs.service
APPLICATION_ID ?= lv.apps.streamrs
DESKTOP_FILE_NAME ?= $(APPLICATION_ID).desktop
DESKTOP_TEMPLATE ?= config/$(APPLICATION_ID).desktop
APPLICATIONS_DIR ?= $(XDG_DATA_HOME)/applications
ICON_SIZE_DIR ?= 512x512
ICON_DEST_DIR ?= $(XDG_DATA_HOME)/icons/hicolor/$(ICON_SIZE_DIR)/apps
ICON_SOURCE ?= config/$(ICON_NAME)
ICON_NAME ?= $(APPLICATION_ID).png
MOCK_OUTPUT ?= mock.png
DEB_VERSION ?= $(shell awk -F '"' '/^version = "/ {print $$2; exit}' Cargo.toml)
DEB_OUTPUT_DIR ?= dist

.PHONY: build install-bin install-systemd install install-config install-images install-desktop install-assets
.PHONY: uninstall-bin uninstall-systemd uninstall-config uninstall-images uninstall-desktop uninstall-assets uninstall
.PHONY: mock deb clean

build:
	cargo build --release --bins

install-bin: build
	mkdir -p "$(BIN_DIR)"
	install -m 0755 "target/release/$(BIN_NAME)" "$(BIN_DIR)/$(BIN_NAME)"
	install -m 0755 "target/release/$(PREVIEW_BIN_NAME)" "$(BIN_DIR)/$(PREVIEW_BIN_NAME)"
	install -m 0755 "target/release/$(GUI_BIN_NAME)" "$(BIN_DIR)/$(GUI_BIN_NAME)"
	install -m 0755 "target/release/$(ICON_COMPOSE_BIN_NAME)" "$(BIN_DIR)/$(ICON_COMPOSE_BIN_NAME)"

install-config:
	mkdir -p "$(CONFIG_DIR)"
	cp "config/default.toml" "$(CONFIG_DIR)/default.toml"

install-images:
	mkdir -p "$(PROFILE_DIR)"
	cp -a all_images/. "$(PROFILE_DIR)/"

install-desktop:
	mkdir -p "$(APPLICATIONS_DIR)"
	rm -f "$(APPLICATIONS_DIR)/streamrs.desktop"
	install -m 0644 "$(DESKTOP_TEMPLATE)" "$(APPLICATIONS_DIR)/$(DESKTOP_FILE_NAME)"
	mkdir -p "$(ICON_DEST_DIR)"
	install -m 0644 "$(ICON_SOURCE)" "$(ICON_DEST_DIR)/$(ICON_NAME)"

install-assets: install-config install-images install-desktop

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
	rm -f "$(BIN_DIR)/$(PREVIEW_BIN_NAME)"
	rm -f "$(BIN_DIR)/$(GUI_BIN_NAME)"
	rm -f "$(BIN_DIR)/$(ICON_COMPOSE_BIN_NAME)"

uninstall-systemd:
	-systemctl --user disable --now "$(SERVICE_NAME).service"
	rm -f "$(SERVICE_FILE)"
	-systemctl --user daemon-reload

uninstall-config:
	rm -f "$(CONFIG_DIR)/$(PROFILE).toml"

uninstall-images:
	rm -rf "$(PROFILE_DIR)"

uninstall-desktop:
	rm -f "$(APPLICATIONS_DIR)/$(DESKTOP_FILE_NAME)"
	rm -f "$(APPLICATIONS_DIR)/streamrs.desktop"
	rm -f "$(ICON_DEST_DIR)/$(ICON_NAME)"

uninstall-assets: uninstall-config uninstall-images uninstall-desktop

uninstall: uninstall-systemd uninstall-assets uninstall-bin

mock:
	cargo run --quiet --bin streamrs-preview -- --output "$(MOCK_OUTPUT)"

deb: build
	./scripts/build-deb.sh "$(DEB_VERSION)" "$(DEB_OUTPUT_DIR)"

clean:
	cargo clean
	@if [ -d dist ] && [ -z "$$(find dist -mindepth 1 -print -quit)" ]; then rmdir dist; fi
