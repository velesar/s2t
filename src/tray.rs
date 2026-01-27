use ksni::{menu::StandardItem, MenuItem, Tray, TrayService};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum TrayAction {
    OpenWindow,
    Quit,
}

pub struct DictationTray {
    tx: Sender<TrayAction>,
}

impl DictationTray {
    pub fn new(tx: Sender<TrayAction>) -> Self {
        Self { tx }
    }

    pub fn spawn_service(tx: Sender<TrayAction>) -> ksni::Handle<Self> {
        let tray_service = TrayService::new(Self::new(tx));
        let handle = tray_service.handle();
        tray_service.spawn();
        handle
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
