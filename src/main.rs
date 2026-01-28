mod audio;
mod channels;
mod conference_recorder;
mod config;
mod context;
mod continuous;
mod dialogs;
mod diarization;
mod history;
mod hotkeys;
mod loopback;
mod models;
mod paste;
mod recordings;
mod ring_buffer;
mod services;
#[cfg(test)]
mod test_support;
mod traits;
mod tray;
mod types;
mod ui;
mod vad;
mod whisper;

use anyhow::Result;
use config::{load_config, models_dir, sortformer_models_dir, Config};
use context::AppContext;
use diarization::DiarizationEngine;
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use gtk4::{glib, prelude::*, Application};
use history::{load_history, save_history, History};
use hotkeys::HotkeyManager;
use models::get_model_path;
use services::TranscriptionService;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tray::{DictationTray, TrayAction};

const APP_ID: &str = "ua.voice.dictation";

fn find_model_path(config: &Config) -> Option<String> {
    let config_model_path = get_model_path(&config.default_model);
    if config_model_path.exists() {
        return Some(config_model_path.to_string_lossy().to_string());
    }

    let fallback_paths = vec![
        models_dir().join("ggml-base.bin"),
        dirs::home_dir()
            .map(|p| p.join(".local/share/whisper/ggml-base.bin"))
            .unwrap_or_default(),
        std::path::PathBuf::from("ggml-base.bin"),
        std::path::PathBuf::from("models/ggml-base.bin"),
        std::path::PathBuf::from("/usr/share/whisper/ggml-base.bin"),
        std::path::PathBuf::from("/usr/local/share/whisper/ggml-base.bin"),
    ];

    for path in fallback_paths {
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }

    None
}

fn main() -> Result<()> {
    gtk4::init()?;

    let config = load_config().unwrap_or_else(|e| {
        eprintln!(
            "Помилка завантаження конфігу: {}. Використовую значення за замовчуванням.",
            e
        );
        Config::default()
    });
    let config = Arc::new(Mutex::new(config));

    // Ensure recordings directory exists
    if let Err(e) = recordings::ensure_recordings_dir() {
        eprintln!("Помилка створення директорії записів: {}", e);
    }

    // Load and cleanup history
    let history = {
        let mut h = load_history().unwrap_or_else(|e| {
            eprintln!("Помилка завантаження історії: {}. Створюю нову.", e);
            History::default()
        });
        let cfg = config.lock().unwrap();
        h.cleanup_old_entries(cfg.history_max_age_days);
        h.trim_to_limit(cfg.history_max_entries);
        drop(cfg);
        if let Err(e) = save_history(&h) {
            eprintln!("Помилка збереження історії: {}", e);
        }
        Arc::new(Mutex::new(h))
    };

    // Initialize transcription service with Whisper model
    let transcription = {
        let cfg = config.lock().unwrap();
        if let Some(model_path) = find_model_path(&cfg) {
            println!("Завантаження моделі: {}", model_path);
            match TranscriptionService::with_model(&model_path) {
                Ok(service) => {
                    println!("Модель завантажено!");
                    service
                }
                Err(e) => {
                    eprintln!("Не вдалося завантажити модель: {}", e);
                    eprintln!("Запустіть додаток і завантажте модель через меню 'Моделі'");
                    TranscriptionService::new()
                }
            }
        } else {
            println!("Модель не знайдено. Завантажте через меню 'Моделі'.");
            TranscriptionService::new()
        }
    };

    // Initialize diarization engine
    let diarization_engine = {
        let cfg = config.lock().unwrap();
        let model_path = if let Some(ref path) = cfg.sortformer_model_path {
            Some(PathBuf::from(path))
        } else {
            // Try default location
            let default_path =
                sortformer_models_dir().join("diar_streaming_sortformer_4spk-v2.1.onnx");
            if default_path.exists() {
                Some(default_path)
            } else {
                None
            }
        };

        let mut engine = DiarizationEngine::new(model_path);
        if let Err(e) = engine.load_model() {
            eprintln!("Не вдалося завантажити модель Sortformer: {}", e);
            eprintln!("Diarization буде використовувати channel-based метод.");
        }
        engine
    };

    // Create AppContext bundling all services
    let ctx = Arc::new(
        AppContext::new(
            config.clone(),
            history.clone(),
            transcription,
            diarization_engine,
        )
        .expect("Failed to create AppContext"),
    );

    let (tray_tx, tray_rx) = async_channel::unbounded();
    let tray_handle =
        DictationTray::spawn_service(tray_tx, config.clone(), ctx.transcription.clone());

    let app = Application::builder().application_id(APP_ID).build();

    // Initialize hotkey manager
    let hotkey_manager = Arc::new(Mutex::new(HotkeyManager::new().unwrap_or_else(|e| {
        eprintln!("Помилка ініціалізації гарячих клавіш: {}", e);
        std::process::exit(1);
    })));

    // Register hotkeys from config
    {
        let cfg = config.lock().unwrap();
        let mut hk = hotkey_manager.lock().unwrap();
        if let Err(e) = hk.register_from_config(&cfg) {
            eprintln!("Помилка реєстрації гарячих клавіш: {}", e);
        }
    }

    // Listen for hotkey reload signals (when settings change)
    let hotkey_manager_for_reload = hotkey_manager.clone();
    let config_for_reload = config.clone();
    let reload_hotkeys_rx = ctx.channels.reload_hotkeys_rx().clone();
    std::thread::spawn(move || {
        while reload_hotkeys_rx.recv_blocking().is_ok() {
            let cfg = config_for_reload.lock().unwrap();
            let mut hk = hotkey_manager_for_reload.lock().unwrap();
            if let Err(e) = hk.register_from_config(&cfg) {
                eprintln!("Помилка перереєстрації гарячих клавіш: {}", e);
            }
        }
    });

    // Listen for hotkey events
    let toggle_recording_tx_for_hotkey = ctx.channels.toggle_recording_tx().clone();
    std::thread::spawn(move || loop {
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state == HotKeyState::Pressed {
                let _ = toggle_recording_tx_for_hotkey.try_send(());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    });

    // Pass AppContext to UI
    let ctx_for_app = ctx.clone();
    app.connect_activate(move |app| {
        ui::build_ui(app, ctx_for_app.clone());
    });

    // Use channels from ctx for tray action handling
    let channels_for_tray = ctx.channels.clone();
    let app_weak = app.downgrade();
    glib::spawn_future_local(async move {
        while let Ok(action) = tray_rx.recv().await {
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
                TrayAction::ManageModels => {
                    if let Some(app) = app_weak.upgrade() {
                        if let Some(window) = app.active_window() {
                            window.present();
                        } else {
                            app.activate();
                        }
                        let _ = channels_for_tray.open_models_tx().try_send(());
                    }
                }
                TrayAction::OpenHistory => {
                    if let Some(app) = app_weak.upgrade() {
                        if let Some(window) = app.active_window() {
                            window.present();
                        } else {
                            app.activate();
                        }
                        let _ = channels_for_tray.open_history_tx().try_send(());
                    }
                }
                TrayAction::OpenSettings => {
                    if let Some(app) = app_weak.upgrade() {
                        if let Some(window) = app.active_window() {
                            window.present();
                        } else {
                            app.activate();
                        }
                        let _ = channels_for_tray.open_settings_tx().try_send(());
                    }
                }
                TrayAction::Quit => {
                    if let Some(app) = app_weak.upgrade() {
                        app.quit();
                    }
                    break;
                }
            }
        }
    });

    app.run();
    tray_handle.shutdown();

    Ok(())
}
