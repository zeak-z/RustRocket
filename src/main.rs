use gio::prelude::*;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Entry, ListBox, ListBoxRow, Label, Separator, LabelBuilder};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::collections::HashSet;
use std::time::SystemTime;
use chrono::prelude::*;

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
        let apps_file = Path::new("apps.txt");

        // Check if the file exists, if not create it and write all apps to it
        if !apps_file.exists() {
            let entries = fs::read_dir(path).unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.metadata().ok().map_or(false, |m| m.permissions().mode() & 0o111 != 0))
                .map(|e| e.file_name().into_string().unwrap_or_default())
                .collect::<Vec<_>>();
            fs::write(&apps_file, entries.join("\n") + "\n---\n").unwrap();
        }

        // Load all apps and recently used apps
        let content = fs::read_to_string(&apps_file).unwrap();
        let parts: Vec<&str> = content.split("---\n").collect();
        let all_apps = parts[0].lines().map(|line| line.to_string()).collect::<Vec<_>>();
        let recent_apps = if parts.len() > 1 { parts[1].lines().map(|line| line.to_string()).collect::<Vec<_>>() } else { vec![] };

        // Load recently used apps
        for app_name in &recent_apps {
            let row = ListBoxRow::new();
            let label = Label::new(Some(app_name));
            row.add(&label);
            list_box.add(&row);
        }
        list_box.show_all();

        let list_box_clone = list_box.clone();
        let all_apps_clone = all_apps.clone();
        entry.connect_changed(move |entry| {
            let input = entry.get_text().as_str().to_string();
            list_box_clone.foreach(|child| {
                list_box_clone.remove(child);
            });

            let entries = all_apps_clone.iter()
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
            let mut recent_apps = fs::read_to_string(&apps_file)
                .unwrap()
                .split("---\n")
                .nth(1)
                .unwrap_or("")
                .lines()
                .map(|line| line.to_string())
                .collect::<HashSet<_>>();
            recent_apps.insert(app_name.clone());
            fs::write(&apps_file, all_apps.join("\n") + "\n---\n" + &recent_apps.into_iter().collect::<Vec<_>>().join("\n")).unwrap();

            let output = Command::new(&app_name)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .expect("Failed to execute command");

            println!("{}", String::from_utf8_lossy(&output.stdout));
            std::process::exit(0); // Add this line to close the application after launching another application
        });

        vbox.pack_start(&entry, false, false, 0);
        vbox.pack_start(&list_box, true, true, 0);

        // Add a separator
        let separator = Separator::new(gtk::Orientation::Horizontal);
        vbox.pack_start(&separator, false, false, 0);

        // Add a label to display the date and time
        let datetime = SystemTime::now();
        let datetime: DateTime<Local> = datetime.into();
        let label = LabelBuilder::new()
            .label(&datetime.format("%I:%M %p %m/%d/%Y").to_string())
            .build();
        vbox.pack_start(&label, false, false, 0);

        window.add(&vbox);

        window.show_all();
    });

    application.run(&[]);
}

