mod steam;
mod ui;

use adw::prelude::*;

fn main() {
    // Create the Libadwaita application.
    // Libadwaita handles the standard GTK4 and Adwaita styles initialization.
    let application = adw::Application::builder()
        .application_id("org.gnome.Vapor")
        .build();

    // Register UI build logic on activation
    application.connect_activate(|app| {
        ui::build_ui(app);
    });

    // Execute application loop
    application.run();
}
