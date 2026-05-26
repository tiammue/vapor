# Vapor

Vapor is a lightweight, modern, GNOME-native companion client for Steam. Built with **Rust**, **GTK4**, and **Libadwaita**, it offers a fast, clean layout to view, launch, and manage your local Steam library alongside the official Steam client.

![Vapor Interface Mockup](https://raw.githubusercontent.com/tiammue/vapor/main/logo.png) *Note: Substitute with real screenshot.*

## Features

- **Responsive Game Grid**: Scales dynamically to your full window resolution (no hardcoded resolution limits) with crisp vertical cover art.
- **Title Text Wrapping**: Long game titles cleanly wrap up to 2 lines and use ending ellipses without stretching columns or breaking grid alignment.
- **Background Steam Monitor**: A live status indicator dot in the header bar showing whether Steam is active. If Steam is offline, the dot turns red and the button allows you to launch Steam directly in the background.
- **Dynamic Playtime & Last Played Info**: Reads playtime and last-played dates directly from Steam's local user configuration VDFs, formatted using native Glib localized datetimes.
- **Focus Auto-Sync & Refresh**: Rescans your Steam directories automatically whenever the application gains focus (focus auto-sync), alongside a manual refresh button.
- **Autocomplete Store Search**: A store search dialog accessible via the `+` header button. Search games on the Steam Store asynchronously and trigger installations instantly.
- **Launch & Uninstall**: Start games instantly using detached detached system commands or uninstall games directly from the details page.
- **Instant Search Filters**: A live search bar in the header bar to filter your local library instantly as you type.

---

## Installation & Prerequisites

Vapor compiles against native GTK4 and Libadwaita development headers. 

### Fedora
To install the required dependencies on Fedora, run:
```bash
sudo dnf install cargo rustc gtk4-devel libadwaita-devel pkg-config
```

### Ubuntu / Debian
```bash
sudo apt install cargo rustc libgtk-4-dev libadwaita-1-dev pkg-config
```

---

## Getting Started

1. **Clone the Repository**:
   ```bash
   git clone https://github.com/tiammue/vapor.git
   cd vapor
   ```

2. **Run the Client**:
   ```bash
   cargo run
   ```

3. **Build Release Binary**:
   ```bash
   cargo build --release
   ```
   The compiled executable will be located at `target/release/vapor`.

---

## Technical Details

- **ACF Parser**: Scans and parses standard and flatpak Steam installation manifests (`appmanifest_*.acf` files) using `keyvalues-serde` directly.
- **Proc FS Scanning**: Reads the Linux `/proc` filesystem directly to identify if the Steam client process is active.
- **Detached Launching**: Launches games using `steam steam://run/{appid}` or `xdg-open` fallback handlers.
- **Asynchronous Search Workers**: Search queries are executed on background threads and channeled back to the main UI thread via standard non-blocking loops, ensuring a lag-free visual experience.
- **Low Memory Overhead**: Scans VDF files via a fast string-scanner rather than parsing massive config files completely into memory, keeping Vapor lightweight.
