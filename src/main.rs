use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;
use std::sync::RwLock;
use std::time::SystemTime;
use chrono::prelude::*;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Entry, ListBox, ListBoxRow, Label, Separator, LabelBuilder, Button, Box, Orientation};
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use bincode::serialize;
use rayon::prelude::*;

static PATH: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("/usr/bin"));
static APPS_FILE: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("apps.bin"));

#[derive(Serialize, Deserialize)]
struct Cache {
    all_apps: HashMap<char, Vec<String>>,
    recent_apps: BTreeSet<String>,
}

static CACHE: Lazy<RwLock<Cache>> = Lazy::new(|| {
    let all_apps: HashMap<char, Vec<String>> = fs::read_dir(&*PATH)
        .unwrap()
        .par_bridge() // Convert to parallel iterator
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if let Ok(metadata) = entry.metadata() {
                if metadata.permissions().mode() & 0o111 != 0 {
                    if let Some(name) = entry.file_name().to_str() {
                        let first_char = name.chars().next().unwrap_or_default().to_ascii_lowercase();
                        let name = name.to_lowercase();
                        return Some((first_char, name));
                    }
                }
            }
            None
        })
        .fold(|| HashMap::new(), |mut map, (first_char, name)| {
            map.entry(first_char).or_insert_with(Vec::new).push(name);
            map
        })
        .reduce(|| HashMap::new(), |mut all_apps, map| {
            for (first_char, names) in map {
                all_apps.entry(first_char).or_insert_with(Vec::new).extend(names);
            }
            all_apps
        });

    let recent_apps = BTreeSet::new();

    RwLock::new(Cache {
        all_apps,
        recent_apps,
    })
});

fn save_cache(cache: &Cache) {
    if let Ok(data) = serialize(cache) {
        fs::write(&*APPS_FILE, data).expect("Failed to write cache");
    }
}

fn create_row(app_name: &str) -> ListBoxRow {
    let row = ListBoxRow::new();
    let label = Label::new(Some(app_name));
    row.add(&label);
    row
}

fn launch_app(app_name: &str, window: &ApplicationWindow) {
    {
        let mut cache = CACHE.write().unwrap();
        cache.recent_apps.insert(app_name.to_string());
        save_cache(&*cache);
    }

    Command::new(app_name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to execute command");
    window.close();
}

fn main() {
    let application = Application::new(Some("com.example.GtkApplication"), Default::default())
        .expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Application Launcher");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let entry = Entry::new();
        let list_box = Rc::new(ListBox::new());

        {
            let cache = CACHE.read().unwrap();
            for app_name in &cache.recent_apps {
                list_box.add(&create_row(app_name));
            }
        }
        list_box.show_all();

        entry.connect_changed(glib::clone!(@strong list_box => move |entry| {
            let input = entry.get_text().to_lowercase();
            list_box.foreach(|child| {
                list_box.remove(child);
            });

            let mut entries: Vec<_> = {
                let cache = CACHE.read().unwrap();
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

        entry.connect_activate(glib::clone!(@strong list_box, @strong window => move |_entry| {
            if let Some(row) = list_box.get_row_at_index(0) {
                if let Some(label) = row.get_child().and_then(|child| child.downcast::<Label>().ok()) {
                    let app_name = label.get_text().to_string();
                    launch_app(&app_name, &window);
                }
            }
        }));

        list_box.connect_row_activated(glib::clone!(@strong window => move |_list_box, row| {
            if let Some(label) = row.get_child().and_then(|child| child.downcast::<Label>().ok()) {
                let app_name = label.get_text().to_string();
                launch_app(&app_name, &window);
            }
        }));

        vbox.pack_start(&entry, false, false, 0);
        vbox.pack_start(list_box.as_ref(), true, true, 0);

        let separator = Separator::new(gtk::Orientation::Horizontal);
        vbox.pack_start(&separator, false, false, 0);

        let datetime: DateTime<Local> = SystemTime::now().into();
        let label = LabelBuilder::new()
            .label(&datetime.format("%I:%M %p %m/%d/%Y").to_string())
            .build();

        let hbox = Box::new(Orientation::Horizontal, 0);
        hbox.pack_start(&label, true, true, 0);

        let power_button = Button::with_label("Power");
        power_button.connect_clicked(|_| {
            Command::new("shutdown")
                .arg("-h")
                .arg("now")
                .spawn()
                .expect("Failed to execute command");
        });
        hbox.pack_start(&power_button, false, false, 0);

        let restart_button = Button::with_label("Restart");
        restart_button.connect_clicked(|_| {
            Command::new("reboot")
                .spawn()
                .expect("Failed to execute command");
        });
        hbox.pack_start(&restart_button, false, false, 0);

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

