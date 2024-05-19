use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::os::unix::fs::PermissionsExt;
use std::sync::RwLock;
use std::time::SystemTime;

use chrono::prelude::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Entry, ListBox, ListBoxRow, Label, Separator, Button, Box as GtkBox, Orientation, ScrolledWindow};
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use bincode::serialize;
use rayon::prelude::*;
use glib::clone;
use dirs;

static PATH: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("/usr/bin"));
static ALL_APPS_FILE: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("all_apps.bin"));
static RECENT_APPS_FILE: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("recent_apps.bin"));

#[derive(Serialize, Deserialize)]
struct AllAppsCache {
    all_apps: HashMap<char, Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct RecentAppsCache {
    recent_apps: BTreeSet<String>,
}

static ALL_APPS_CACHE: Lazy<RwLock<AllAppsCache>> = Lazy::new(|| {
    let all_apps: HashMap<char, Vec<String>> = fs::read_dir(&*PATH)
        .expect("Failed to read directory")
        .par_bridge()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if metadata.permissions().mode() & 0o111 == 0 {
                return None;
            }
            let name = entry.file_name().into_string().ok()?;
            let first_char = name.chars().next()?.to_ascii_lowercase();
            Some((first_char, name.to_lowercase()))
        })
        .fold(HashMap::new, |mut map: HashMap<char, Vec<String>>, (first_char, name)| {
            map.entry(first_char).or_default().push(name);
            map
        })
        .reduce(HashMap::new, |mut all_apps, map| {
            for (first_char, names) in map {
                all_apps.entry(first_char).or_default().extend(names);
            }
            all_apps
        });

    RwLock::new(AllAppsCache { all_apps })
});

static RECENT_APPS_CACHE: Lazy<RwLock<RecentAppsCache>> = Lazy::new(|| {
    let recent_apps: BTreeSet<String> = if RECENT_APPS_FILE.exists() {
        let data = fs::read(&*RECENT_APPS_FILE).expect("Failed to read recent apps file");
        bincode::deserialize(&data).expect("Failed to deserialize recent apps data")
    } else {
        BTreeSet::new()
    };
    RwLock::new(RecentAppsCache { recent_apps })
});

fn save_cache<T: Serialize>(file: &PathBuf, cache: &T) -> Result<(), Box<dyn std::error::Error>> {
    let data = serialize(cache)?;
    fs::write(file, data)?;
    Ok(())
}

fn create_row(app_name: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    let label = Label::new(Some(app_name));
    label.set_halign(gtk::Align::Start);
    row.add(&label);
    row
}

fn launch_app(app_name: &str, window: &ApplicationWindow) -> Result<(), Box<dyn std::error::Error>> {
    {
        let mut cache = RECENT_APPS_CACHE.write().map_err(|e| format!("Lock error: {:?}", e))?;
        cache.recent_apps.insert(app_name.to_string());
        save_cache(&RECENT_APPS_FILE, &*cache)?;
    }

    let home_dir = dirs::home_dir().ok_or("Failed to find home directory")?;

    Command::new(app_name)
        .current_dir(home_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    window.close();
    Ok(())
}

fn handle_activate(_entry: &Entry, list_box: &ListBox, window: &ApplicationWindow) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(row) = list_box.get_row_at_index(0) {
        if let Some(label) = row.get_child().and_then(|child| child.downcast::<Label>().ok()) {
            let app_name = label.get_text().to_string();
            launch_app(&app_name, window)?;
        }
    }
    Ok(())
}

fn main() {
    let application = Application::new(Some("com.example.GtkApplication"), Default::default())
        .expect("Failed to initialize GTK application");

    application.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Application Launcher");
        window.set_default_size(300, 200); // Set the default size of the window

        let vbox = GtkBox::new(Orientation::Vertical, 1);

        let entry = Entry::new();

        let scrolled_window = ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);

        let list_box = ListBox::new();

        {
            let cache = RECENT_APPS_CACHE.read().expect("Failed to acquire read lock");
            for app_name in &cache.recent_apps {
                list_box.add(&create_row(app_name));
            }
        }
        list_box.show_all();

        entry.connect_changed(clone!(@strong list_box => move |entry| {
            let input = entry.get_text().to_lowercase();
            list_box.foreach(|child| list_box.remove(child));

            let mut entries: Vec<_> = {
                let cache = ALL_APPS_CACHE.read().expect("Failed to acquire read lock");
                cache.all_apps.values()
                    .flatten()
                    .filter(|app_name| app_name.contains(&input))
                    .cloned()
                    .collect()
            };

            entries.sort();
            entries.truncate(9);

            for app_name in entries {
                list_box.add(&create_row(&app_name));
            }

            list_box.show_all();
        }));

        let entry_clone = entry.clone();
        entry.connect_activate(clone!(@strong list_box, @strong window => move |_entry| {
            if let Err(err) = handle_activate(&entry_clone, &list_box, &window) {
                eprintln!("Failed to handle activate: {}", err);
            }
        }));

        list_box.connect_row_activated(clone!(@strong window => move |_list_box, row| {
            if let Some(label) = row.get_child().and_then(|child| child.downcast::<Label>().ok()) {
                let app_name = label.get_text().to_string();
                if let Err(err) = launch_app(&app_name, &window) {
                    eprintln!("Failed to launch app: {}", err);
                }
            }
        }));

        scrolled_window.add(&list_box);

        vbox.pack_start(&entry, false, false, 0);
        vbox.pack_start(&scrolled_window, true, true, 0);

        let separator = Separator::new(Orientation::Horizontal);
        vbox.pack_start(&separator, false, false, 0);

        let datetime: DateTime<Local> = SystemTime::now().into();
        let label = Label::new(Some(&datetime.format("%I:%M %p %m/%d/%Y").to_string()));
        label.set_halign(gtk::Align::Start);
        vbox.pack_start(&label, false, false, 0);

        let hbox = GtkBox::new(Orientation::Horizontal, 0);

        let power_button = Button::with_label("Power");
        power_button.connect_clicked(|_| {
            Command::new("shutdown")
                .arg("-h")
                .arg("now")
                .spawn()
                .expect("Failed to execute shutdown command");
        });
        hbox.pack_start(&power_button, false, false, 0);

        let restart_button = Button::with_label("Restart");
        restart_button.connect_clicked(|_| {
            Command::new("reboot")
                .spawn()
                .expect("Failed to execute reboot command");
        });
        hbox.pack_start(&restart_button, false, false, 0);

        let logout_button = Button::with_label("Logout");
        logout_button.connect_clicked(|_| {
            Command::new("logout")
                .spawn()
                .expect("Failed to execute logout command");
        });
        hbox.pack_start(&logout_button, false, false, 0);

        vbox.pack_start(&hbox, false, false, 0);

        window.add(&vbox);
        window.show_all();
    });

    application.run(&[]);
}

