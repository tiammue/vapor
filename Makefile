PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
APPDIR ?= $(PREFIX)/share/applications
ICONDIR ?= $(PREFIX)/share/icons/hicolor/512x512/apps

.PHONY: all build install uninstall clean

all: build

build:
	cargo build --release

install:
	mkdir -p $(DESTDIR)$(BINDIR)
	mkdir -p $(DESTDIR)$(APPDIR)
	mkdir -p $(DESTDIR)$(ICONDIR)
	install -m 755 target/release/vapor $(DESTDIR)$(BINDIR)/vapor
	install -m 644 assets/vapor.desktop $(DESTDIR)$(APPDIR)/vapor.desktop
	install -m 644 assets/vapor.png $(DESTDIR)$(ICONDIR)/vapor.png
	@echo "Vapor installed successfully to $(PREFIX)!"
	@echo "Please ensure $(BINDIR) is in your PATH. You may need to restart your shell or desktop session for the menu entry to appear."

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/vapor
	rm -f $(DESTDIR)$(APPDIR)/vapor.desktop
	rm -f $(DESTDIR)$(ICONDIR)/vapor.png
	@echo "Vapor uninstalled successfully from $(PREFIX)."
