use crate::config::{save_config, Config};
use crate::models::{get_model_path, list_downloaded_models};
use crate::whisper::WhisperSTT;
use ksni::{
    menu::{StandardItem, SubMenu},
    MenuItem, Tray, TrayService,
};
use async_channel::Sender;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum TrayAction {
    OpenWindow,
    ManageModels,
    Quit,
}

pub struct DictationTray {
    tx: Sender<TrayAction>,
    config: Arc<Mutex<Config>>,
    whisper: Arc<Mutex<Option<WhisperSTT>>>,
}

impl DictationTray {
    pub fn new(
        tx: Sender<TrayAction>,
        config: Arc<Mutex<Config>>,
        whisper: Arc<Mutex<Option<WhisperSTT>>>,
    ) -> Self {
        Self { tx, config, whisper }
    }

    pub fn spawn_service(
        tx: Sender<TrayAction>,
        config: Arc<Mutex<Config>>,
        whisper: Arc<Mutex<Option<WhisperSTT>>>,
    ) -> ksni::Handle<Self> {
        let tray_service = TrayService::new(Self::new(tx, config, whisper));
        let handle = tray_service.handle();
        tray_service.spawn();
        handle
    }

    fn select_model(&mut self, filename: &str) {
        {
            let mut cfg = self.config.lock().unwrap();
            cfg.default_model = filename.to_string();
            if let Err(e) = save_config(&cfg) {
                eprintln!("Помилка збереження конфігу: {}", e);
                return;
            }
        }

        let model_path = get_model_path(filename);
        match WhisperSTT::new(model_path.to_str().unwrap_or_default()) {
            Ok(new_whisper) => {
                let mut w = self.whisper.lock().unwrap();
                *w = Some(new_whisper);
                println!("Модель завантажено: {}", filename);
            }
            Err(e) => {
                eprintln!("Помилка завантаження моделі: {}", e);
            }
        }
    }
}

impl Tray for DictationTray {
    fn icon_name(&self) -> String {
        "audio-input-microphone".to_string()
    }

    fn title(&self) -> String {
        "Голосова диктовка".to_string()
    }

    fn id(&self) -> String {
        "voice-dictation".to_string()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let downloaded_models = list_downloaded_models();
        let current_model = {
            let cfg = self.config.lock().unwrap();
            cfg.default_model.clone()
        };

        let mut model_items: Vec<MenuItem<Self>> = downloaded_models
            .iter()
            .map(|model| {
                let filename = model.filename.clone();
                let is_current = model.filename == current_model;
                let label = if is_current {
                    format!("• {}", model.display_name)
                } else {
                    format!("  {}", model.display_name)
                };
                StandardItem {
                    label,
                    activate: Box::new(move |tray: &mut Self| {
                        tray.select_model(&filename);
                    }),
                    ..Default::default()
                }
                .into()
            })
            .collect();

        if model_items.is_empty() {
            model_items.push(
                StandardItem {
                    label: "(Немає завантажених моделей)".to_string(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        vec![
            StandardItem {
                label: "Відкрити диктовку".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayAction::OpenWindow);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            SubMenu::<Self> {
                label: "Модель".to_string(),
                submenu: model_items,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Керування моделями...".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayAction::ManageModels);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Вийти".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayAction::OpenWindow);
    }
}
