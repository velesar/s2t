mod audio;
mod tray;
mod ui;
mod whisper;

use anyhow::Result;
use gtk4::{glib, prelude::*, Application};
use std::sync::Arc;
use std::time::Duration;

use tray::{DictationTray, TrayAction};
use whisper::WhisperSTT;

const APP_ID: &str = "ua.voice.dictation";

fn get_model_path() -> Result<String> {
    let paths = vec![
        dirs::data_local_dir()
            .map(|p| p.join("whisper/ggml-base.bin"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|p| p.join(".local/share/whisper/ggml-base.bin"))
            .unwrap_or_default(),
        std::path::PathBuf::from("ggml-base.bin"),
        std::path::PathBuf::from("models/ggml-base.bin"),
        std::path::PathBuf::from("/usr/share/whisper/ggml-base.bin"),
        std::path::PathBuf::from("/usr/local/share/whisper/ggml-base.bin"),
    ];

    for path in paths {
        if path.exists() {
            return Ok(path.to_string_lossy().to_string());
        }
    }

    Err(anyhow::anyhow!(
        r#"Модель Whisper не знайдено!

Завантажте модель командою:
    mkdir -p ~/.local/share/whisper
    curl -L -o ~/.local/share/whisper/ggml-base.bin \
        https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

Для кращої якості можна завантажити більшу модель:
    ggml-small.bin  (~500MB) - краща якість
    ggml-medium.bin (~1.5GB) - ще краща
    ggml-large-v3.bin (~3GB) - найкраща"#
    ))
}

fn main() -> Result<()> {
    gtk4::init()?;

    let model_path = get_model_path()?;
    println!("Завантаження моделі: {}", model_path);

    let whisper = Arc::new(WhisperSTT::new(&model_path)?);
    println!("Модель завантажено!");

    let (tray_tx, tray_rx) = std::sync::mpsc::channel();
    let tray_handle = DictationTray::spawn_service(tray_tx);

    let app = Application::builder().application_id(APP_ID).build();

    let whisper_for_app = whisper.clone();
    app.connect_activate(move |app| {
        ui::build_ui(app, whisper_for_app.clone());
    });

    let app_weak = app.downgrade();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if let Ok(action) = tray_rx.try_recv() {
            match action {
                TrayAction::OpenWindow => {
                    if let Some(app) = app_weak.upgrade() {
                        if let Some(window) = app.active_window() {
                            window.present();
                        } else {
                            app.activate();
                        }
                    }
                }
                TrayAction::Quit => {
                    if let Some(app) = app_weak.upgrade() {
                        app.quit();
                    }
                    return glib::ControlFlow::Break;
                }
            }
        }
        glib::ControlFlow::Continue
    });

    app.run();
    tray_handle.shutdown();

    Ok(())
}
