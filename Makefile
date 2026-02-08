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
MOCK_SCRIPT ?= scripts/mock_preview.py
MOCK_TEMPLATE ?= scripts/blank.svg
MOCK_CONFIG ?= $(CONFIG_DIR)/$(PROFILE).toml
MOCK_IMAGE_DIR ?= $(PROFILE_DIR)
MOCK_OUTPUT ?= mock.png
MOCK_SMALL_OUTPUT ?= dist/mock-small.png
MOCK_WIDTH ?= 1560
MOCK_HEIGHT ?= 1108
MOCK_ICON_INSET ?= 8
MOCK_BOTTOM_ROW_Y_OFFSET ?= 0
MOCK_BOTTOM_ROW_EXTRA_INSET ?= 1
MOCK_ICON_CONTENT_SHRINK_X ?= 10
MOCK_ICON_CONTENT_SHRINK_Y ?= 10
MOCK_ICON_MASK_EXPAND ?= 10

.PHONY: build install-bin install-systemd install install-config install-images install-assets
.PHONY: uninstall-bin uninstall-systemd uninstall-config uninstall-images uninstall-assets uninstall
.PHONY: mock mock-small

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

mock:
	python3 "$(MOCK_SCRIPT)" \
		--blank-svg "$(MOCK_TEMPLATE)" \
		--config "$(MOCK_CONFIG)" \
		--image-dir "$(MOCK_IMAGE_DIR)" \
		--output "$(MOCK_OUTPUT)" \
		--width "$(MOCK_WIDTH)" \
		--height "$(MOCK_HEIGHT)" \
		--icon-inset "$(MOCK_ICON_INSET)" \
		--bottom-row-y-offset "$(MOCK_BOTTOM_ROW_Y_OFFSET)" \
		--bottom-row-extra-inset "$(MOCK_BOTTOM_ROW_EXTRA_INSET)" \
		--icon-content-shrink-x "$(MOCK_ICON_CONTENT_SHRINK_X)" \
		--icon-content-shrink-y "$(MOCK_ICON_CONTENT_SHRINK_Y)" \
		--icon-mask-expand "$(MOCK_ICON_MASK_EXPAND)"

mock-small: mock
	mkdir -p "$(dir $(MOCK_SMALL_OUTPUT))"
	magick "$(MOCK_OUTPUT)" -resize 780x554 "$(MOCK_SMALL_OUTPUT)"
