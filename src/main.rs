#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{process::exit, fs::{read_to_string, File}, io::Write, time::Duration, thread};

use crossbeam_channel::{Sender, Receiver, unbounded};
use discord_rich_presence::{activity::{Activity, Assets, Party}, DiscordIpc, DiscordIpcClient as Client};
use eframe::{egui::{CentralPanel, Context, Slider, Layout, menu, Button}, App, Frame, epaint::{Vec2, Color32}, emath::Align};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
struct DisPresenceConfig {
    app_id: String,
    details: String,
    state: String,
    party: (u32, u32),
    image_large: (String, String),
    image_small: (String, String),
}

struct DisPresenceApp {
    config_name: Option<String>,
    config_path: Option<String>,
    config: DisPresenceConfig,
    config_temp: DisPresenceConfig,
    rx: Receiver<String>,
    tx: Sender<String>,
    threaded: bool,
}

impl App for DisPresenceApp {
    fn update(
        &mut self,
        ctx: &Context,
        _frame: &mut Frame,
    ) -> () {
        CentralPanel::default().show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    ui.menu_button("config", |ui| {
                        if ui.button("load").clicked() {
                            if let Some(path) = FileDialog::new()
                                .add_filter("configuration file", &["json", "dspson"])
                                .pick_file()
                            {
                                self.config_name = Some(path.file_name().unwrap().to_string_lossy().to_string());
                                self.config_path = Some(path.display().to_string());

                                if self.config_path.is_some() {
                                    let serialized = read_to_string(self.config_path.as_ref().unwrap()).unwrap();
                                    self.config = serde_json::from_str(serialized.as_str()).unwrap();
                                    self.config_temp = self.config.clone();
                                }
                            }
                        }
                    });
                    
                    if ui.button("exit").clicked() {
                        exit(0);
                    }
                });

                ui.menu_button("?", |ui| {
                    ui.hyperlink_to("repo", "https://github.com/Ciavi/dispresence-rs");
                });
            });

            ui.add_space(12.0);

            ui.group(|ui| {
                if let Some(name) = &self.config_name {
                    ui.horizontal(|ui| {
                        ui.label("loaded: ");
                        ui.monospace(name);
                    });
                } else {
                    ui.label("load config (file > config > load)");
                }
    
                ui.label("application_id: ");
                ui.text_edit_singleline(&mut self.config_temp.app_id);
            });

            ui.add_space(12.0);

            ui.group(|ui| {
                ui.label("details: ");
                ui.text_edit_multiline(&mut self.config_temp.details);
    
                ui.label("state: ");
                ui.text_edit_singleline(&mut self.config_temp.state);
            });

            ui.add_space(12.0);

            ui.group(|ui| {    
                ui.horizontal_wrapped(|ui| {
                    ui.label("party: ");
                    ui.monospace(format!("{}/{}", self.config_temp.party.0, self.config_temp.party.1));
                });

                ui.add(Slider::new(&mut self.config_temp.party.0, 1..=10));
                ui.add(Slider::new(&mut self.config_temp.party.1, self.config_temp.party.0..=10));
            });

            ui.add_space(12.0);
            
            ui.group(|ui| {
                ui.label("large_image_key: ");
                ui.text_edit_singleline(&mut self.config_temp.image_large.0);
    
                ui.label("large_image_text: ");
                ui.text_edit_singleline(&mut self.config_temp.image_large.1);
            });

            ui.add_space(12.0);

            ui.group(|ui| {
                ui.label("small_image_key: ");
                ui.text_edit_singleline(&mut self.config_temp.image_small.0);

                ui.label("small_image_text: ");
                ui.text_edit_singleline(&mut self.config_temp.image_small.1);
            });

            ui.add_space(12.0);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.config_path.is_some() && self.config == self.config_temp {
                        if self.threaded {
                            if ui.add(Button::new("stop").fill(Color32::from_rgb(153, 0, 0))).clicked() {
                                loop {
                                    if let Ok(()) = self.tx.send("END".to_string()) {
                                        self.threaded = false;
                                        return;
                                    }
                                }
                            }
                        } else {
                            if ui.add(Button::new("apply").fill(Color32::from_rgb(39, 78, 19))).clicked() {
                                let config = self.config.clone();
                                let mut client = Client::new(&config.app_id).unwrap();
    
                                let crx = self.rx.clone();
    
                                thread::spawn(move || {
                                    loop {
                                        if client.connect().is_ok() {
                                            break;
                                        }
                                    }
        
                                    loop {
                                        let mut activity = Activity::<'static>::new()
                                            .details(&config.details)
                                            .state(&config.state);
        
                                        if config.party != (0, 0) && config.party != (1, 1) {
                                            activity = activity.party(Party::new().size([
                                                config.party.0.try_into().unwrap(),
                                                config.party.1.try_into().unwrap(),
                                            ]));
                                        }
        
                                        if config.image_large != ("".to_string(), "".to_string()) && config.image_large != ("none".to_string(), "none".to_string()) {
                                            activity = activity.assets(Assets::new()
                                                .large_image(&config.image_large.0)
                                                .large_text(&config.image_large.1)
                                            );
                                        }
                                            
                                        if config.image_small != ("".to_string(), "".to_string()) && config.image_small != ("none".to_string(), "none".to_string()) {
                                            activity = activity.assets(Assets::new()
                                                .small_image(&config.image_small.0)
                                                .small_text(&config.image_small.1)
                                            );
                                        }
                                            
                                        if client.set_activity(activity).is_err() && client.reconnect().is_ok() {
                                            continue;
                                        }
        
                                        match crx.try_recv() {
                                            Ok(_) => return,
                                            Err(_) => {},
                                        };
                                
                                        thread::sleep(Duration::from_secs(15));
                                    }
                                });
    
                                self.threaded = true;
                            }
                        }
                    } else {
                        ui.add_enabled(false, Button::new("apply"));
                    }

                    if ui.button("save").clicked() {
                        let serialized = serde_json::to_string(&self.config_temp).unwrap();
                        self.config = self.config_temp.clone();

                        let mut file: File = match &self.config_path {
                            Some(path) => File::create(path).unwrap(),
                            None => {
                                if let Some(path) = FileDialog::new()
                                    .add_filter("configuration file", &["json", "dspson"])
                                    .save_file()
                                {
                                    File::create(path).unwrap()
                                } else {
                                    File::create("./config.json").unwrap()
                                }
                            }
                        };

                        file.set_len(0).unwrap();
                        file.write_all(serialized.as_bytes()).unwrap();
                    }
                })
            });
        });
    }
}

fn main() {
    let options = eframe::NativeOptions {
        always_on_top: true,
        resizable: false,
        min_window_size: Some(Vec2::new(300.0, 580.0)),
        max_window_size: Some(Vec2::new(300.0, 600.0)),
        ..Default::default()
    };

    let (ttx, rrx) = unbounded::<String>();

    eframe::run_native(
        "DisPresence",
        options,
        Box::new(|_cct| Box::new(DisPresenceApp {
            config_name: None,
            config_path: None,
            config: DisPresenceConfig::default(),
            config_temp: DisPresenceConfig::default(),
            rx: rrx,
            tx: ttx,
            threaded: false,
        }))
    );
}
