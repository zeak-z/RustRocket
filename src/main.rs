use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Entry, ListBox, ListBoxRow, Label};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::io::{BufRead, BufReader, Write};
use std::collections::HashSet;

fn main() {
    let application = Application::new(
        Some("com.example.GtkApplication"),
        Default::default(),
    ).expect("failed to initialize GTK application");

    application.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title("Application Launcher");
        window.set_default_size(350, 70);

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 1);
        let entry = Entry::new();
        let list_box = ListBox::new();
        let path = Path::new("/usr/bin");
        let recent_apps_file = Path::new("app_cache.txt");
        let all_apps_file = Path::new("all_apps.txt");

        // Check if the file exists, if not create it
        if !recent_apps_file.exists() {
            fs::File::create(&recent_apps_file).expect("Could not create file");
        }

        // Check if the file exists, if not create it and write all apps to it
        if !all_apps_file.exists() {
            let entries = fs::read_dir(path).unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.metadata().ok().map_or(false, |m| m.permissions().mode() & 0o111 != 0))
                .map(|e| e.file_name().into_string().unwrap_or_default())
                .collect::<Vec<_>>();
            fs::write(&all_apps_file, entries.join("\n")).unwrap();
        }

        // Load all apps
        let all_apps = fs::read_to_string(&all_apps_file)
            .unwrap()
            .lines()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();

        // Load recently used apps
        let file = fs::File::open(&recent_apps_file).unwrap();
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let app_name = line.unwrap_or_default();
            let row = ListBoxRow::new();
            let label = Label::new(Some(&app_name));
            row.add(&label);
            list_box.add(&row);
        }
        list_box.show_all();

        let list_box_clone = list_box.clone();
        entry.connect_changed(move |entry| {
            let input = entry.get_text().as_str().to_string();
            list_box_clone.foreach(|child| {
                list_box_clone.remove(child);
            });

            let entries = all_apps.iter()
                .filter(|app_name| app_name.contains(&input))
                .collect::<Vec<_>>();

            for app_name in entries {
                let row = ListBoxRow::new();
                let label = Label::new(Some(app_name));
                row.add(&label);
                list_box_clone.add(&row);
            }
            list_box_clone.show_all();
        });

        list_box.connect_row_activated(move |_list_box, row| {
            let label = row.get_child().unwrap().downcast::<Label>().unwrap();
            let app_name = label.get_text().as_str().to_string();

            // Update recently used apps
            let mut recent_apps = fs::read_to_string(&recent_apps_file)
                .unwrap()
                .lines()
                .map(|line| line.to_string())
                .collect::<HashSet<_>>();
            recent_apps.insert(app_name.clone());
            fs::write(&recent_apps_file, recent_apps.into_iter().collect::<Vec<_>>().join("\n")).unwrap();

            let output = Command::new(&app_name)
                .output()
                .expect("Failed to execute command");

            println!("{}", String::from_utf8_lossy(&output.stdout));
            std::process::exit(0); // Add this line to close the application after launching another application
        });

        vbox.pack_start(&entry, false, false, 0);
        vbox.pack_start(&list_box, true, true, 0);
        window.add(&vbox);

        window.show_all();
    });

    application.run(&[]);
}

