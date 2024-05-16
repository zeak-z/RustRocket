use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;
use chrono::prelude::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Entry, ListBox, ListBoxRow, Label, Separator, LabelBuilder, Button, Box, Orientation};
use once_cell::sync::Lazy;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::collections::HashMap;
use lazy_static::lazy_static;

lazy_static! {
    static ref PATH: PathBuf = PathBuf::from("/usr/bin");
    static ref APPS_FILE: PathBuf = PathBuf::from("apps.txt");
}

static ALL_APPS: Lazy<HashMap<char, Vec<String>>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for entry in fs::read_dir(&*PATH).unwrap_or_else(|_| panic!("Failed to read directory")) {
        if let Ok(entry) = entry {
            if let Ok(metadata) = entry.metadata() {
                if metadata.permissions().mode() & 0o111 != 0 {
                    if let Some(name) = entry.file_name().to_str() {
                        let first_char = name.chars().next().unwrap_or_default().to_ascii_lowercase();
                        map.entry(first_char).or_insert_with(Vec::new).push(name.to_string());
                    }
                }
            }
        }
    }
    map
});

static RECENT_APPS: Lazy<HashSet<String>> = Lazy::new(|| {
    fs::read_to_string(&*APPS_FILE).ok()
        .and_then(|content| {
            content.split("---\n").nth(1).map(|apps| {
                apps.lines().map(|line| line.to_string()).collect()
            })
        })
        .unwrap_or_default()
});

fn create_row(app_name: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    let label = Label::new(Some(app_name));
    row.add(&label);
    row
}

fn launch_app(app_name: &str) {
    // Update recently used apps
    let mut recent_apps = fs::read_to_string(&*APPS_FILE)
        .unwrap_or_default()
        .split("---\n")
        .nth(1)
        .unwrap_or("")
        .lines()
        .map(|line| line.to_string())
        .collect::<HashSet<_>>();
    recent_apps.insert(app_name.to_string());
    fs::write(&*APPS_FILE, ALL_APPS.values().flatten().cloned().collect::<Vec<_>>().join("\n") + "\n---\n" + &recent_apps.into_iter().collect::<Vec<_>>().join("\n")).unwrap();

    let output = Command::new(app_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .expect("Failed to execute command");

    println!("{}", String::from_utf8_lossy(&output.stdout));
    std::process::exit(0); // Add this line to close the application after launching another application
}

fn main() {
    let application = Application::new(
        Some("com.example.GtkApplication"),
        Default::default(),
    ).expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Application Launcher");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let entry = Entry::new();
        let list_box = Rc::new(ListBox::new());

        for app_name in RECENT_APPS.iter() {
            list_box.add(&create_row(app_name));
        }
        list_box.show_all();

        entry.connect_changed(glib::clone!(@strong list_box => move |entry| {
            let input = entry.get_text().as_str().to_lowercase();
            list_box.foreach(|child| {
                list_box.remove(child);
            });

            let mut entries = ALL_APPS.values()
                .flatten()
                .filter(|app_name| app_name.to_lowercase().contains(&input))
                .cloned()
                .collect::<Vec<_>>();

            // Limit the number of results to 9
            entries.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
            entries.truncate(9);

            for app_name in entries {
                list_box.add(&create_row(&app_name));
            }

            list_box.show_all();
        }));

        entry.connect_activate(glib::clone!(@strong list_box => move |_entry| {
            if let Some(row) = list_box.get_row_at_index(0) {
                if let Some(label) = row.get_child().and_then(|child| child.downcast::<Label>().ok()) {
                    let app_name = label.get_text().as_str().to_string();
                    launch_app(&app_name);
                }
            }
        }));

        list_box.connect_row_activated(move |list_box, row| {
            if let Some(label) = row.get_child().and_then(|child| child.downcast::<Label>().ok()) {
                let app_name = label.get_text().as_str().to_string();
                launch_app(&app_name);
            }
        });

        vbox.pack_start(&entry, false, false, 0);
        vbox.pack_start(list_box.as_ref(), true, true, 0);

        // Add a separator
        let separator = Separator::new(gtk::Orientation::Horizontal);
        vbox.pack_start(&separator, false, false, 0);

        // Add a label to display the date and time
        let datetime = SystemTime::now();
        let datetime: DateTime<Local> = datetime.into();
        let label = LabelBuilder::new()
            .label(&datetime.format("%I:%M %p %m/%d/%Y").to_string())
            .build();

        // Create a horizontal box to hold the label and buttons
        let hbox = Box::new(Orientation::Horizontal, 0);
        hbox.pack_start(&label, true, true, 0);

        // Create the power button
        let power_button = Button::with_label("Power");
        power_button.connect_clicked(|_| {
            Command::new("shutdown")
                .arg("-h")
                .arg("now")
                .spawn()
                .expect("Failed to execute command");
        });
        hbox.pack_start(&power_button, false, false, 0);

        // Create the restart button
        let restart_button = Button::with_label("Restart");
        restart_button.connect_clicked(|_| {
            Command::new("reboot")
                .spawn()
                .expect("Failed to execute command");
        });
        hbox.pack_start(&restart_button, false, false, 0);

        // Create the logout button
        let logout_button = Button::with_label("Logout");
        logout_button.connect_clicked(|_| {
            Command::new("logout")
                .spawn()
                .expect("Failed to execute command");
        });
        hbox.pack_start(&logout_button, false, false, 0);

        vbox.pack_start(&hbox, false, false, 0);

        window.add(&vbox);

        window.show_all();
    });

    application.run(&[]);
}

