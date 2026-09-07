mod auth;
mod auth_ely;
mod config;
#[cfg(windows)]
mod dpapi;
mod game;
mod mods;
mod news;
mod rules;
mod server_list;
mod server_status;
mod settings;
mod shell;
mod skin;
mod skin_elyby;
mod update;
mod violations;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_deep_link::DeepLinkExt;

// Must build on the main thread; this fn must stay async or it can deadlock on Windows
#[tauri::command]
async fn open_logs_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("logs") {
        let _ = existing.set_focus();
        return Ok(());
    }

    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let app_handle = app.clone();
    app
        .run_on_main_thread(move || {
            let result = (|| {
                WebviewWindowBuilder::new(&app_handle, "logs", WebviewUrl::App("index.html".into()))
                    .title("Логи игры — LemiCraft")
                    .inner_size(640.0, 480.0)
                    .decorations(false)
                    .build()
                    .map_err(|err| err.to_string())?;
                Ok(())
            })();
            let _ = tx.send(result);
        })
        .map_err(|err| err.to_string())?;

    rx.await.map_err(|_| "Канал закрылся до ответа от главного потока".to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(game::GameProcess::default())
    .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
      if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
      }
    }))
    .plugin(tauri_plugin_deep_link::init())
    .plugin(tauri_plugin_dialog::init())
    // Blocks WebView2 shortcuts (Ctrl+R, Ctrl+W...); PlatformOptions is the only part that works on Windows
    .plugin({
      let builder = tauri_plugin_prevent_default::Builder::new().with_flags(tauri_plugin_prevent_default::Flags::all());
      #[cfg(windows)]
      let builder = builder.platform(
        tauri_plugin_prevent_default::PlatformOptions::new()
          .browser_accelerator_keys(false)
          .default_context_menus(false),
      );
      builder.build()
    })
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // macOS registers the scheme at bundle time (Info.plist); Windows/Linux need this at runtime,
      // including for our portable (non-installed) exe, which has no installer step to do it
      #[cfg(any(windows, target_os = "linux"))]
      let _ = app.deep_link().register_all();

      let handle = app.handle().clone();
      app.deep_link().on_open_url(move |event| {
        if let Some(url) = event.urls().into_iter().next() {
          let _ = handle.emit("deep-link-url", url.to_string());
        }
      });

      // Cold start via URL (app wasn't running yet) — on_open_url alone won't cover this
      if let Ok(Some(urls)) = app.deep_link().get_current() {
        if let Some(url) = urls.into_iter().next() {
          let _ = app.emit("deep-link-url", url.to_string());
        }
      }

      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      game::play,
      game::stop_game,
      game::is_game_running,
      game::read_current_log,
      game::move_game_files,
      auth::login_microsoft,
      auth_ely::login_elyby,
      auth_ely::cancel_elyby_login,
      auth::get_current_account,
      auth::logout,
      server_status::get_server_status,
      server_status::get_cached_server_status,
      settings::get_settings,
      settings::save_settings,
      news::get_news,
      news::get_cached_news,
      news::get_more_news,
      shell::open_external,
      game::open_game_folder,
      game::get_default_game_dir,
      game::is_game_installed,
      settings::get_total_ram_gb,
      skin::fetch_skin_data_uri,
      skin::get_current_account_skins,
      skin::get_cached_current_account_skins,
      skin::upload_current_account_skin,
      skin::apply_current_account_skin,
      skin::delete_current_account_skin,
      skin_elyby::has_elyby_skin_session,
      skin_elyby::clear_elyby_skin_session,
      skin_elyby::open_elyby_web_login,
      skin_elyby::finish_elyby_web_login,
      skin_elyby::get_elyby_skins,
      skin_elyby::get_cached_elyby_skins,
      skin_elyby::wear_elyby_skin,
      skin_elyby::take_off_elyby_skin,
      skin_elyby::delete_elyby_skin,
      skin_elyby::upload_elyby_skin,
      update::check_for_update,
      update::download_and_install_update,
      mods::get_cached_mod_catalog,
      mods::get_mod_catalog,
      mods::get_installed_mods,
      mods::export_mod_selection,
      mods::preview_import_code,
      mods::install_mods,
      mods::apply_import_extras,
      mods::uninstall_mod,
      mods::get_official_pack,
      mods::get_installed_official_pack_version,
      mods::apply_official_pack,
      mods::uninstall_official_pack,
      rules::get_cached_rules,
      rules::get_rules,
      violations::get_violations,
      open_logs_window
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
