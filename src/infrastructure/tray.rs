use crate::app::config::{save_config, Config};
use crate::infrastructure::models::{get_model_path, list_downloaded_models};
use crate::transcription::TranscriptionService;
use crate::domain::traits::Transcription;
use async_channel::Sender;
use ksni::{
    menu::{StandardItem, SubMenu},
    MenuItem, Tray, TrayMethods,
};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum TrayAction {
    OpenWindow,
    ManageModels,
    OpenHistory,
    OpenSettings,
    Quit,
}

pub struct DictationTray {
    tx: Sender<TrayAction>,
    config: Arc<Mutex<Config>>,
    transcription: Arc<Mutex<TranscriptionService>>,
}

impl DictationTray {
    pub fn new(
        tx: Sender<TrayAction>,
        config: Arc<Mutex<Config>>,
        transcription: Arc<Mutex<TranscriptionService>>,
    ) -> Self {
        Self {
            tx,
            config,
            transcription,
        }
    }

    pub async fn spawn_service(
        tx: Sender<TrayAction>,
        config: Arc<Mutex<Config>>,
        transcription: Arc<Mutex<TranscriptionService>>,
    ) -> Result<ksni::Handle<Self>, ksni::Error> {
        Self::new(tx, config, transcription).spawn().await
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
        let mut ts = self.transcription.lock().unwrap();
        if let Err(e) = ts.load_model(&model_path) {
            eprintln!("Помилка завантаження моделі: {}", e);
        } else {
            println!("Модель завантажено: {}", filename);
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
                    let _ = tray.tx.try_send(TrayAction::OpenWindow);
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
                    let _ = tray.tx.try_send(TrayAction::ManageModels);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Історія...".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.try_send(TrayAction::OpenHistory);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Налаштування...".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.try_send(TrayAction::OpenSettings);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Вийти".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.try_send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.try_send(TrayAction::OpenWindow);
    }
}
