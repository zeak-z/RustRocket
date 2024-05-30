use std::{fs, process::Command, sync::Mutex, time::SystemTime};
use std::collections::VecDeque;
use std::path::PathBuf;
use xdg::BaseDirectories;
use chrono::prelude::*;
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use bincode::{serialize, deserialize};
use dirs;
use eframe::egui;
use eframe::egui::{CentralPanel, Context, ScrollArea, TextEdit, CursorIcon};

static RECENT_APPS_FILE: Lazy<PathBuf> = Lazy::new(|| PathBuf::from("recent_apps.bin"));

#[derive(Serialize, Deserialize)]
struct RecentAppsCache { recent_apps: VecDeque<String> }

fn save_cache<T: Serialize>(file: &PathBuf, cache: &T) -> Result<(), Box<dyn std::error::Error>> {
    let data = serialize(cache)?;
    fs::write(file, data)?;
    Ok(())
}

static RECENT_APPS_CACHE: Lazy<Mutex<RecentAppsCache>> = Lazy::new(|| {
    let recent_apps = if RECENT_APPS_FILE.exists() {
        let data = fs::read(&*RECENT_APPS_FILE).expect("Failed to read recent apps file");
        deserialize(&data).expect("Failed to deserialize recent apps data")
    } else { VecDeque::new() };
    Mutex::new(RecentAppsCache { recent_apps })
});

fn get_desktop_entries() -> Vec<String> {
    let xdg_dirs = BaseDirectories::new().unwrap();
    let data_dirs = xdg_dirs.get_data_dirs();

    let mut entries = Vec::new();
    for dir in data_dirs {
        let desktop_files = dir.join("applications");
        if let Ok(entries_iter) = fs::read_dir(desktop_files) {
            for entry in entries_iter {
                if let Ok(entry) = entry {
                    if let Some(path) = entry.path().to_str() {
                        if path.ends_with(".desktop") {
                            entries.push(path.to_string());
                        }
                    }
                }
            }
        }
    }
    entries
}

fn parse_desktop_entry(path: &str) -> Option<(String, String)> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    for line in content.lines() {
        if line.starts_with("Name=") {
            name = Some(line[5..].to_string());
        } else if line.starts_with("Exec=") {
            exec = Some(line[5..].to_string());
        }
        if name.is_some() && exec.is_some() {
            break;
        }
    }
    if let (Some(name), Some(exec)) = (name, exec) {
        let cleaned_exec = exec.replace("%f", "")
                               .replace("%u", "")
                               .replace("%U", "")
                               .replace("%F", "")
                               .replace("%i", "")
                               .replace("%c", "")
                               .replace("%k", "")
                               .trim()
                               .to_string();
        Some((name, cleaned_exec))
    } else {
        None
    }
}

fn search_applications(query: &str, applications: &[(String, String)]) -> Vec<(String, String)> {
    applications
        .iter()
        .filter(|(name, _)| name.to_lowercase().contains(&query.to_lowercase()))
        .cloned()
        .collect()
}

fn launch_app(app_name: &str, exec_cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cache = RECENT_APPS_CACHE.lock().map_err(|e| format!("Lock error: {:?}", e))?;
    cache.recent_apps.retain(|x| x != app_name);
    cache.recent_apps.push_front(app_name.to_string());
    if cache.recent_apps.len() > 10 { cache.recent_apps.pop_back(); }
    save_cache(&RECENT_APPS_FILE, &*cache)?;

    let home_dir = dirs::home_dir().ok_or("Failed to find home directory")?;
    Command::new("sh")
        .arg("-c")
        .arg(exec_cmd)
        .current_dir(home_dir)
        .spawn()?;
    Ok(())
}

struct AppLauncher {
    query: String,
    applications: Vec<(String, String)>,
    search_results: Vec<(String, String)>,
    is_quit: bool,
}

impl Default for AppLauncher {
    fn default() -> Self {
        let applications: Vec<(String, String)> = get_desktop_entries()
            .iter()
            .filter_map(|path| parse_desktop_entry(path))
            .collect();

        let recent_apps_cache = RECENT_APPS_CACHE.lock().expect("Failed to acquire read lock");

        Self {
            query: String::new(),
            search_results: recent_apps_cache.recent_apps.iter().filter_map(|app_name| {
                applications.iter().find(|(name, _)| name == app_name).cloned()
            }).collect(),
            applications,
            is_quit: false,
        }
    }
}

impl eframe::App for AppLauncher {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();  // Ensure the app continues to repaint while it's running
        
        if self.is_quit {
            std::process::exit(0);  // Exit the process if is_quit is true
        }

        ctx.input(|i| {
            if i.key_pressed(egui::Key::Escape) {
                self.is_quit = true;
            }
            if i.key_pressed(egui::Key::Enter) {
                if let Some((app_name, exec_cmd)) = self.search_results.first() {
                    if let Err(err) = launch_app(app_name, exec_cmd) {
                        eprintln!("Failed to launch app: {}", err);
                    } else {
                        self.is_quit = true;
                    }
                }
            }
        });

        CentralPanel::default().show(ctx, |ui| {
            ui.heading("Application Launcher");
            if ui.add(TextEdit::singleline(&mut self.query).hint_text("Search...")).changed() {
                self.search_results = search_applications(&self.query, &self.applications);
            }

            ScrollArea::vertical().show(ui, |ui| {
                for (app_name, exec_cmd) in &self.search_results {
                    if ui.button(app_name).clicked() {
                        if let Err(err) = launch_app(app_name, exec_cmd) {
                            eprintln!("Failed to launch app: {}", err);
                        } else {
                            self.is_quit = true;
                        }
                    }
                }
            });

            ui.separator();
            
            let datetime: DateTime<Local> = SystemTime::now().into();
            ui.label(datetime.format("%I:%M %p %m/%d/%Y").to_string());

            ui.horizontal(|ui| {
                if ui.button("Power").clicked() {
                    Command::new("shutdown").arg("-h").arg("now").spawn().expect("Failed to execute shutdown command");
                }
                if ui.button("Restart").clicked() {
                    Command::new("reboot").spawn().expect("Failed to execute reboot command");
                }
                if ui.button("Logout").clicked() {
                    Command::new("logout").spawn().expect("Failed to execute logout command");
                }
            });
        });

        // Ensure the cursor is set correctly on every frame
        ctx.output_mut(|o| o.cursor_icon = CursorIcon::Default);
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        // Removed window size options
        ..Default::default()
    };
    eframe::run_native(
        "Application Launcher",
        native_options,
        Box::new(|_cc| Box::new(AppLauncher::default())),
    )
}

