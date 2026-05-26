use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::steam;

/// Appends custom CSS styling to the application to deliver a premium, modern design.
fn load_custom_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(r#"
        /* Vapor Native Stylesheet */
        
        .game-card {
            background-color: alpha(@theme_fg_color, 0.04);
            border: 1px solid alpha(@theme_fg_color, 0.08);
            border-radius: 16px;
            padding: 16px;
            margin: 6px;
            min-width: 140px;

            transition: background-color 200ms, border-color 200ms;
        }
        
        .game-card:hover {
            background-color: alpha(@theme_fg_color, 0.08);
            border-color: @theme_selected_bg_color;
        }
        
        .game-card-icon {
            color: @theme_selected_bg_color;
            opacity: 0.85;
        }
        
        .game-card:hover .game-card-icon {
            opacity: 1.0;
        }
        
        .game-cover {
            border-radius: 12px;
            box-shadow: 0 4px 10px rgba(0, 0, 0, 0.25);
            background-color: alpha(@theme_fg_color, 0.03);
        }
        
        .game-cover-detail {
            border-radius: 16px;
            box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
            margin-bottom: 8px;
        }
        
        .detail-view {
            padding: 48px;
        }
        
        .detail-icon {
            color: @theme_selected_bg_color;
            margin-bottom: 12px;
        }
        
        .play-button {
            font-size: 1.15rem;
            font-weight: bold;
            padding: 16px 52px;
        }

        .uninstall-button {
            background-color: alpha(@destructive_color, 0.1);
            color: @destructive_color;
            font-size: 1.15rem;
            font-weight: bold;
            padding: 16px 52px;
            transition: background-color 200ms, color 200ms;
        }
        
        .uninstall-button:hover {
            background-color: @destructive_color;
            color: @destructive_fg_color;
        }
    "#);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Helper to walk a FlowBox child's widgets to extract the game card's title label.
fn get_flow_child_title(child: &gtk::FlowBoxChild) -> Option<String> {
    let card_box = child.child()?.downcast::<gtk::Box>().ok()?;
    let mut current = card_box.first_child();
    let mut label_text = None;
    while let Some(widget) = current {
        if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
            label_text = Some(label.text().to_string());
        }
        current = widget.next_sibling();
    }
    label_text
}

/// Helper to render the modal dialog for searching and installing Steam games.
fn show_install_dialog(parent: &gtk::Window, refresh_callback: Rc<dyn Fn()>) {
    let dialog = gtk::Window::builder()
        .title("Search & Install Games")
        .modal(true)
        .transient_for(parent)
        .default_width(450)
        .default_height(400)
        .build();

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content_box.set_margin_top(16);
    content_box.set_margin_bottom(16);
    content_box.set_margin_start(16);
    content_box.set_margin_end(16);

    let title_label = gtk::Label::builder()
        .label("<span weight=\"bold\" size=\"large\">Search Steam Store</span>")
        .use_markup(true)
        .halign(gtk::Align::Start)
        .build();
    content_box.append(&title_label);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search game by name (e.g. Portal, Half-Life)...")
        .build();
    content_box.append(&search_entry);

    let info_label = gtk::Label::builder()
        .label("Type game title and press Enter to search.")
        .halign(gtk::Align::Start)
        .css_classes(vec!["caption".to_string()])
        .build();
    content_box.append(&info_label);

    // ListBox for search results
    let list_box = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(vec!["boxed-list".to_string()])
        .build();

    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .child(&list_box)
        .build();
    
    scrolled_window.set_size_request(-1, 200);
    content_box.append(&scrolled_window);

    // Manual AppID install box below
    let manual_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    manual_box.set_margin_top(8);
    let manual_entry = gtk::Entry::builder()
        .placeholder_text("Or enter App ID manually...")
        .hexpand(true)
        .build();
    manual_box.append(&manual_entry);

    let manual_install_btn = gtk::Button::builder()
        .label("Install ID")
        .css_classes(vec!["suggested-action".to_string()])
        .build();
    manual_box.append(&manual_install_btn);
    content_box.append(&manual_box);

    let close_btn = gtk::Button::builder()
        .label("Close")
        .halign(gtk::Align::End)
        .margin_top(8)
        .build();
    content_box.append(&close_btn);

    dialog.set_child(Some(&content_box));

    // Close action
    let dialog_clone = dialog.clone();
    close_btn.connect_clicked(move |_| {
        dialog_clone.close();
    });

    // Manual Install Action
    let dialog_clone = dialog.clone();
    let refresh_callback_clone = refresh_callback.clone();
    let manual_entry_clone = manual_entry.clone();
    manual_install_btn.connect_clicked(move |_| {
        let appid = manual_entry_clone.text().to_string();
        if !appid.trim().is_empty() {
            if let Err(e) = steam::install_game(&appid) {
                eprintln!("Failed to trigger installation: {}", e);
            } else {
                refresh_callback_clone();
            }
            dialog_clone.close();
        }
    });

    // Set up channel for thread communication
    let (sender, receiver) = std::sync::mpsc::channel::<Vec<steam::SearchResultItem>>();
    let receiver_cell = Rc::new(std::cell::RefCell::new(receiver));

    // Search trigger action (user hits Enter in search_entry)
    let search_entry_clone = search_entry.clone();
    let info_label_clone = info_label.clone();
    search_entry.connect_activate(move |_| {
        let query = search_entry_clone.text().to_string();
        if !query.trim().is_empty() {
            info_label_clone.set_text("Searching Steam Store...");
            let sender_clone = sender.clone();
            std::thread::spawn(move || {
                match steam::search_games(&query) {
                    Ok(results) => {
                        let _ = sender_clone.send(results);
                    }
                    Err(e) => {
                        eprintln!("Search thread error: {}", e);
                        let _ = sender_clone.send(Vec::new());
                    }
                }
            });
        }
    });

    // Receive search results via non-blocking polling timer on main thread
    let list_box_clone = list_box.clone();
    let info_label_clone2 = info_label.clone();
    let dialog_clone2 = dialog.clone();
    let dialog_weak = dialog.downgrade();
    let refresh_callback_clone2 = refresh_callback.clone();
    let receiver_cell_clone = receiver_cell.clone();

    gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        // Check if dialog still exists, otherwise stop timer to prevent memory leaks
        let _dialog = match dialog_weak.upgrade() {
            Some(d) => d,
            None => return gtk::glib::ControlFlow::Break,
        };

        if let Ok(results) = receiver_cell_clone.borrow().try_recv() {
            // Clear old results
            while let Some(child) = list_box_clone.first_child() {
                list_box_clone.remove(&child);
            }

            if results.is_empty() {
                info_label_clone2.set_text("No games found or search failed. Try another query.");
            } else {
                info_label_clone2.set_text(&format!("Found {} results:", results.len()));
                
                for item in results {
                    let row = adw::ActionRow::builder()
                        .title(&item.name)
                        .subtitle(&format!("App ID: {}", item.id))
                        .build();

                    let install_btn = gtk::Button::builder()
                        .label("Install")
                        .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
                        .valign(gtk::Align::Center)
                        .build();

                    let id_str = item.id.to_string();
                    let dialog_clone_inner = dialog_clone2.clone();
                    let refresh_clone_inner = refresh_callback_clone2.clone();
                    install_btn.connect_clicked(move |_| {
                        if let Err(e) = steam::install_game(&id_str) {
                            eprintln!("Failed to trigger installation for AppID {}: {}", id_str, e);
                        } else {
                            refresh_clone_inner();
                        }
                        dialog_clone_inner.close();
                    });

                    row.add_suffix(&install_btn);
                    list_box_clone.append(&row);
                }
            }
        }

        gtk::glib::ControlFlow::Continue
    });

    dialog.present();
}

/// Builds the master user interface and assigns it to the Application window.
pub fn build_ui(app: &adw::Application) {
    // Load styling rules
    load_custom_css();

    // View stack serves as our root view container to toggle views
    let view_stack = adw::ViewStack::new();

    // --- 1. Library Grid View ---
    let library_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    
    // Header Bar
    let header_bar = adw::HeaderBar::new();
    let window_title = adw::WindowTitle::new("Vapor", "Vapor Steam Client");
    header_bar.set_title_widget(Some(&window_title));
    
    // Add "Install" Button in headerbar
    let install_trigger_btn = gtk::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Install New Game")
        .build();
    header_bar.pack_start(&install_trigger_btn);

    // Add "Refresh" Button in headerbar
    let refresh_btn = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh Library")
        .build();
    header_bar.pack_start(&refresh_btn);

    // Add Steam status button
    let steam_status_btn = gtk::Button::builder()
        .tooltip_text("Steam Status")
        .build();
    let steam_status_label = gtk::Label::builder()
        .use_markup(true)
        .build();
    steam_status_btn.set_child(Some(&steam_status_label));
    header_bar.pack_start(&steam_status_btn);

    // Add local filter entry in headerbar
    let filter_entry = gtk::SearchEntry::builder()
        .placeholder_text("Filter installed games...")
        .max_width_chars(20)
        .build();
    header_bar.pack_end(&filter_entry);
    
    library_box.append(&header_bar);

    // Responsive layout container
    let scrolled_window = gtk::ScrolledWindow::new();
    scrolled_window.set_hscrollbar_policy(gtk::PolicyType::Never);
    scrolled_window.set_vscrollbar_policy(gtk::PolicyType::Automatic);
    scrolled_window.set_vexpand(true);
    scrolled_window.set_hexpand(true);

    let flow_box = gtk::FlowBox::new();
    flow_box.set_valign(gtk::Align::Start);
    flow_box.set_halign(gtk::Align::Fill);
    flow_box.set_selection_mode(gtk::SelectionMode::None);
    flow_box.set_min_children_per_line(2);
    flow_box.set_max_children_per_line(6);
    flow_box.set_column_spacing(18);
    flow_box.set_row_spacing(18);
    flow_box.set_margin_top(24);
    flow_box.set_margin_bottom(24);
    flow_box.set_margin_start(24);
    flow_box.set_margin_end(24);

    scrolled_window.set_child(Some(&flow_box));
    library_box.append(&scrolled_window);

    // --- 2. Game Details View ---
    let detail_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Detail Header Bar
    let detail_header_bar = adw::HeaderBar::new();
    let detail_window_title = adw::WindowTitle::new("", "");
    detail_header_bar.set_title_widget(Some(&detail_window_title));

    // Detail Back Button
    let back_button = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Back to Library")
        .build();
    detail_header_bar.pack_start(&back_button);
    detail_box.append(&detail_header_bar);

    // Detail Pane Content
    let detail_content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    detail_content.add_css_class("detail-view");
    detail_content.set_valign(gtk::Align::Center);
    detail_content.set_halign(gtk::Align::Center);
    detail_content.set_vexpand(true);
    detail_content.set_hexpand(true);

    // Dynamic Image Container to load poster covers or iconic fallback
    let detail_image_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    detail_content.append(&detail_image_container);

    let game_title_label = gtk::Label::builder()
        .use_markup(true)
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(vec!["title-1".to_string()])
        .build();
    detail_content.append(&game_title_label);

    let appid_label = gtk::Label::builder()
        .css_classes(vec!["caption".to_string()])
        .build();
    detail_content.append(&appid_label);

    let stats_label = gtk::Label::builder()
        .css_classes(vec!["caption".to_string()])
        .build();
    detail_content.append(&stats_label);

    // Play & Uninstall buttons wrapped together
    let button_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    button_row.set_halign(gtk::Align::Center);
    button_row.set_margin_top(16);

    let play_button = gtk::Button::builder()
        .label("Play Game")
        .css_classes(vec!["suggested-action".to_string(), "pill".to_string(), "play-button".to_string()])
        .build();
    button_row.append(&play_button);

    let uninstall_button = gtk::Button::builder()
        .label("Uninstall")
        .css_classes(vec!["destructive-action".to_string(), "pill".to_string(), "uninstall-button".to_string()])
        .build();
    button_row.append(&uninstall_button);

    detail_content.append(&button_row);

    let detail_clamp = adw::Clamp::new();
    detail_clamp.set_maximum_size(600);
    detail_clamp.set_child(Some(&detail_content));
    detail_box.append(&detail_clamp);

    // --- 3. View Management Setup ---
    view_stack.add_named(&library_box, Some("library"));
    view_stack.add_named(&detail_box, Some("detail"));

    // Rc / RefCell holds the ID of the game actively viewed
    let active_appid = Rc::new(RefCell::new(None::<String>));

    // Dynamic library list reloader closure
    let refresh_library = {
        let flow_box = flow_box.clone();
        let scrolled_window = scrolled_window.clone();
        let view_stack = view_stack.clone();
        let active_appid = active_appid.clone();
        let game_title_label = game_title_label.clone();
        let appid_label = appid_label.clone();
        let stats_label = stats_label.clone();
        let detail_window_title = detail_window_title.clone();
        let detail_image_container = detail_image_container.clone();
        
        move || {
            // Remove existing cards
            while let Some(child) = flow_box.first_child() {
                flow_box.remove(&child);
            }
            
            let games = steam::scan_steam_games();
            
            if games.is_empty() {
                let status_page = adw::StatusPage::new();
                status_page.set_title("No Games Found");
                status_page.set_description(Some("Vapor could not locate any Steam app manifests on your system.\nMake sure Steam is installed and games are loaded on your disk."));
                status_page.set_icon_name(Some("input-gaming-symbolic"));
                scrolled_window.set_child(Some(&status_page));
            } else {
                scrolled_window.set_child(Some(&flow_box));
                for game in games {
                    let card_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
                    card_box.add_css_class("game-card");
                    card_box.set_focusable(true);
                    card_box.set_halign(gtk::Align::Center);
                    card_box.set_valign(gtk::Align::Start);

                    // Load cover poster if cached locally, fallback to game-controller icon
                    if let Some(cover_path) = steam::find_game_cover(&game.appid) {
                        let picture = gtk::Picture::for_filename(&cover_path);
                        picture.set_size_request(130, 195); // 2:3 aspect ratio
                        picture.set_keep_aspect_ratio(true);
                        picture.set_hexpand(false);
                        picture.set_vexpand(false);
                        picture.add_css_class("game-cover");
                        card_box.append(&picture);
                    } else {
                        let card_icon = gtk::Image::builder()
                            .icon_name("input-gaming-symbolic")
                            .pixel_size(48)
                            .css_classes(vec!["game-card-icon".to_string()])
                            .build();

                        let fallback_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
                        fallback_box.set_size_request(130, 195);
                        fallback_box.set_valign(gtk::Align::Center);
                        fallback_box.set_halign(gtk::Align::Center);
                        fallback_box.append(&card_icon);
                        card_box.append(&fallback_box);
                    }

                    let card_label = gtk::Label::builder()
                        .label(&game.name)
                        .wrap(true)
                        .lines(2)
                        .justify(gtk::Justification::Center)
                        .halign(gtk::Align::Center)
                        .ellipsize(gtk::pango::EllipsizeMode::End)
                        .max_width_chars(15)
                        .width_request(130)
                        .build();
                    card_box.append(&card_label);

                    let gesture = gtk::GestureClick::new();
                    let game_clone = game.clone();
                    let view_stack_clone = view_stack.clone();
                    let detail_window_title_clone = detail_window_title.clone();
                    let game_title_label_clone = game_title_label.clone();
                    let appid_label_clone = appid_label.clone();
                    let stats_label_clone = stats_label.clone();
                    let active_appid_clone = active_appid.clone();
                    let detail_image_container_clone = detail_image_container.clone();

                    gesture.connect_pressed(move |_, _, _, _| {
                        *active_appid_clone.borrow_mut() = Some(game_clone.appid.clone());
                        detail_window_title_clone.set_title(&game_clone.name);
                        game_title_label_clone.set_markup(&format!("<span size=\"xx-large\" weight=\"bold\">{}</span>", game_clone.name));
                        appid_label_clone.set_text(&format!("Steam App ID: {}", game_clone.appid));

                        if let Some(stats) = steam::get_game_stats(&game_clone.appid) {
                            let hours = stats.playtime_mins as f32 / 60.0;
                            let playtime_str = if hours > 0.0 {
                                format!("{:.1} hours played", hours)
                            } else {
                                "Never played".to_string()
                            };

                            let last_played_str = if stats.last_played_timestamp > 0 {
                                if let Ok(dt) = gtk::glib::DateTime::from_unix_local(stats.last_played_timestamp as i64) {
                                    dt.format("%b %d, %Y").map(|s| s.to_string()).unwrap_or_else(|_| "Unknown".into())
                                } else {
                                    "Unknown".to_string()
                                }
                            } else {
                                "Never".to_string()
                            };

                            stats_label_clone.set_text(&format!("{}  •  Last played: {}", playtime_str, last_played_str));
                        } else {
                            stats_label_clone.set_text("No playtime data found");
                        }
                        
                        // Clear detail view image container and load poster/icon dynamically
                        while let Some(child) = detail_image_container_clone.first_child() {
                            detail_image_container_clone.remove(&child);
                        }

                        if let Some(cover_path) = steam::find_game_cover(&game_clone.appid) {
                            let detail_pic = gtk::Picture::for_filename(&cover_path);
                            detail_pic.set_size_request(180, 270); // Larger 2:3 detail view poster
                            detail_pic.add_css_class("game-cover-detail");
                            detail_image_container_clone.append(&detail_pic);
                        } else {
                            let detail_icon = gtk::Image::builder()
                                .icon_name("input-gaming-symbolic")
                                .pixel_size(96)
                                .css_classes(vec!["detail-icon".to_string()])
                                .build();
                            detail_image_container_clone.append(&detail_icon);
                        }

                        view_stack_clone.set_visible_child_name("detail");
                    });
                    card_box.add_controller(gesture);

                    let child_widget = gtk::FlowBoxChild::new();
                    child_widget.set_child(Some(&card_box));
                    child_widget.set_halign(gtk::Align::Center);
                    child_widget.set_valign(gtk::Align::Start);
                    
                    flow_box.insert(&child_widget, -1);
                }
            }
        }
    };

    let refresh_library = Rc::new(refresh_library);

    // Set up native FlowBox grid filtering
    let filter_entry_clone = filter_entry.clone();
    flow_box.set_filter_func(move |child| {
        let query = filter_entry_clone.text().to_lowercase();
        if query.is_empty() {
            return true;
        }
        if let Some(title) = get_flow_child_title(child) {
            title.to_lowercase().contains(&query)
        } else {
            true
        }
    });

    // Invalidate filter when query changes
    let flow_box_clone = flow_box.clone();
    filter_entry.connect_search_changed(move |_| {
        flow_box_clone.invalidate_filter();
    });

    // Connect Back Button action
    let view_stack_clone = view_stack.clone();
    let refresh_library_clone = refresh_library.clone();
    back_button.connect_clicked(move |_| {
        view_stack_clone.set_visible_child_name("library");
        refresh_library_clone();
    });

    // Connect Play Button execution
    let active_appid_clone = active_appid.clone();
    let play_button_clone = play_button.clone();
    play_button.connect_clicked(move |_| {
        if let Some(ref appid) = *active_appid_clone.borrow() {
            play_button_clone.set_sensitive(false);
            play_button_clone.set_label("Launching...");

            if let Err(e) = steam::launch_game(appid) {
                eprintln!("Failed to launch game: {}", e);
                play_button_clone.set_label("Failed to Launch");
            }

            let play_btn = play_button_clone.clone();
            gtk::glib::timeout_add_local(std::time::Duration::from_secs(4), move || {
                play_btn.set_sensitive(true);
                play_btn.set_label("Play Game");
                gtk::glib::ControlFlow::Break
            });
        }
    });

    // Connect Uninstall Button execution
    let active_appid_clone = active_appid.clone();
    let view_stack_clone = view_stack.clone();
    let refresh_library_clone = refresh_library.clone();
    uninstall_button.connect_clicked(move |_| {
        if let Some(ref appid) = *active_appid_clone.borrow() {
            if let Err(e) = steam::uninstall_game(appid) {
                eprintln!("Failed to uninstall game: {}", e);
            } else {
                view_stack_clone.set_visible_child_name("library");
                refresh_library_clone();
            }
        }
    });

    // Run initial scan to populate library
    refresh_library();

    // Connect Refresh button
    let refresh_library_clone = refresh_library.clone();
    refresh_btn.connect_clicked(move |_| {
        refresh_library_clone();
    });

    // Steam status monitor setup
    let update_steam_status = {
        let steam_status_btn = steam_status_btn.clone();
        let steam_status_label = steam_status_label.clone();
        move || {
            let running = steam::is_steam_running();
            steam_status_btn.set_sensitive(true); // Keep sensitive so tooltips show on hover
            if running {
                steam_status_label.set_markup("<span color='#2ec27e'>●</span> Steam");
                steam_status_btn.set_tooltip_text(Some("Steam client is active"));
            } else {
                steam_status_label.set_markup("<span color='#e01b24'>●</span> Start Steam");
                steam_status_btn.set_tooltip_text(Some("Steam is offline. Click to launch."));
            }
        }
    };
    let update_steam_status = Rc::new(update_steam_status);

    let update_steam_status_clone = update_steam_status.clone();
    steam_status_btn.connect_clicked(move |_| {
        if !steam::is_steam_running() {
            if let Err(e) = steam::launch_steam() {
                eprintln!("Failed to launch Steam: {}", e);
            } else {
                update_steam_status_clone();
            }
        }
    });

    // Run status check initially and every 5 seconds
    update_steam_status();
    let update_steam_status_clone = update_steam_status.clone();
    gtk::glib::timeout_add_local(std::time::Duration::from_secs(5), move || {
        update_steam_status_clone();
        gtk::glib::ControlFlow::Continue
    });

    // --- 5. Application Window Initialization ---
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Vapor")
        .default_width(850)
        .default_height(600)
        .content(&view_stack)
        .build();

    // Focus / Active window auto-sync connection
    let refresh_library_clone = refresh_library.clone();
    window.connect_notify_local(Some("is-active"), move |win, _| {
        if win.is_active() {
            refresh_library_clone();
        }
    });

    // Connect Install Dialog Trigger Button
    let window_weak = window.downgrade();
    let refresh_library_clone2 = refresh_library.clone();
    install_trigger_btn.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            show_install_dialog(window.upcast_ref(), refresh_library_clone2.clone());
        }
    });

    window.present();
}
