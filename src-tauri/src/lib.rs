mod espn_proxy;
mod release_check;

use std::{sync::OnceLock, time::Duration};

use reqwest::{Client, redirect::Policy};
use tauri::AppHandle;

#[cfg(target_os = "android")]
use jni::{
    EnvUnowned,
    errors::ThrowRuntimeExAndDefault,
    jni_mangle,
    objects::{JClass, JObject},
};

static ESPN_CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();

fn espn_client() -> Result<&'static Client, String> {
    ESPN_CLIENT
        .get_or_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(20))
                .redirect(Policy::custom(|attempt| {
                    if !espn_proxy::should_follow_redirect(attempt.url(), attempt.previous().len())
                    {
                        attempt.stop()
                    } else {
                        attempt.follow()
                    }
                }))
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

#[tauri::command]
async fn fetch_espn(url: String) -> Result<serde_json::Value, String> {
    espn_proxy::fetch_espn_with(&url, |parsed_url| async move {
        let response = espn_client()?
            .get(parsed_url)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let content_length = response.content_length();
        espn_proxy::validate_response_metadata(status, content_length)?;

        let mut body = Vec::new();
        let mut response = response;
        while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
            espn_proxy::append_response_chunk(&mut body, &chunk)?;
        }

        Ok(espn_proxy::EspnResponse {
            status,
            content_length,
            body,
        })
    })
    .await
}

#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Result<Option<release_check::AppUpdate>, String> {
    let installed_version = app.package_info().version.to_string();
    let current_version =
        release_check::version_for_update_check(&installed_version, cfg!(debug_assertions));
    release_check::check_for_update(&current_version, release_check::Platform::current()?).await
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[jni_mangle("com.ryan.sportsscoreboard.MainActivity")]
pub fn initializeRustlsPlatformVerifier<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) {
    unowned_env
        .with_env(|env| rustls_platform_verifier::android::init_with_env(env, context))
        .resolve::<ThrowRuntimeExAndDefault>();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![fetch_espn, check_for_updates])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
