//! OS notifications are an optional native provider. Merely registering it never
//! prompts for permission or sends a notification.
use super::*;

#[derive(Clone, serde::Serialize)]
struct PermissionAttempt {
    state: &'static str,
    error_code: Option<isize>,
}
type Attempt = Arc<Mutex<Option<PermissionAttempt>>>;
fn attempt() -> Attempt {
    static STATE: std::sync::OnceLock<Attempt> = std::sync::OnceLock::new();
    STATE.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}
fn begin_attempt(state: &Attempt) -> Result<()> {
    let mut current = state
        .lock()
        .map_err(|_| anyhow::anyhow!("Notification request state unavailable"))?;
    if current.as_ref().is_some_and(|a| a.state == "pending") {
        bail!(
            "conflict: notification authorization is already pending; respond to the OS request or check notification settings"
        );
    }
    *current = Some(PermissionAttempt {
        state: "pending",
        error_code: None,
    });
    Ok(())
}
fn finish_attempt(state: &Attempt, granted: bool, error_code: Option<isize>) {
    if let Ok(mut state) = state.lock() {
        *state = Some(PermissionAttempt {
            state: if error_code.is_some() {
                "failed"
            } else if granted {
                "granted"
            } else {
                "denied"
            },
            error_code,
        });
    }
}

pub(super) fn supported() -> std::result::Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if objc2_foundation::NSBundle::mainBundle()
            .bundleIdentifier()
            .is_none()
        {
            return Err("Notifications require a macOS app bundle with an identifier".into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Notification provider currently requires macOS".into())
    }
}

pub(super) async fn invoke(app: tauri::AppHandle, command: &str, input: Value) -> Result<Value> {
    supported().map_err(anyhow::Error::msg)?;
    #[cfg(target_os = "macos")]
    {
        macos::invoke(app, command, input).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, command, input);
        bail!("unsupported: notification provider currently requires macOS");
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSArray, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
        UNNotificationRequest, UNNotificationSettings, UNUserNotificationCenter,
    };
    use std::ptr::NonNull;

    fn identifier(value: &Value) -> std::result::Result<String, String> {
        let id = value["id"].as_str().ok_or("Notification id required")?;
        if id.is_empty()
            || id.len() > 120
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
        {
            return Err("Invalid notification id".into());
        }
        Ok(format!("thesaurus.{id}"))
    }
    async fn settings(app: &tauri::AppHandle) -> Result<Value> {
        let (send, recv) = tokio::sync::oneshot::channel();
        let send = Mutex::new(Some(send));
        on_main(app.clone(),move|_| {
            let block=RcBlock::new(move|settings:NonNull<UNNotificationSettings>| {
                // SAFETY: Apple's completion handler supplies a live settings
                // object for the duration of this callback. No ObjC object is
                // transferred into Rust's async runtime.
                let settings=unsafe {settings.as_ref()};
                let state=match settings.authorizationStatus() {
                    UNAuthorizationStatus::NotDetermined=>"not_determined",UNAuthorizationStatus::Denied=>"denied",UNAuthorizationStatus::Authorized=>"authorized",UNAuthorizationStatus::Provisional=>"provisional",UNAuthorizationStatus::Ephemeral=>"ephemeral",_=>"unknown",
                };
                let attempt=attempt().lock().ok().and_then(|s|s.clone());
                let detail=match attempt.as_ref().map(|a|a.state) {
                    Some("pending")=>"macOS has not completed the permission request. Respond to its prompt or review this app in OS notification settings.",
                    Some("failed")=>"macOS could not request notification authorization for this app. Check the app installation and OS notification settings.",
                    Some("denied")=>"Notification permission was not granted. You can review this app in OS notification settings.",
                    _=>"OS settings checked. Alert presentation also depends on Focus and notification settings.",
                };
                if let Ok(mut send)=send.lock() {if let Some(send)=send.take(){let _=send.send(json!({"permission":state,"alert_setting":settings.alertSetting().0,"sound_setting":settings.soundSetting().0,"permission_request":attempt,"detail":detail}));}}
            });
            UNUserNotificationCenter::currentNotificationCenter().getNotificationSettingsWithCompletionHandler(&block);Ok(())
        }).await?;
        tokio::time::timeout(std::time::Duration::from_secs(3), recv)
            .await
            .context("Notification settings check timed out")?
            .context("Notification settings check was interrupted")
    }
    pub(super) async fn invoke(
        app: tauri::AppHandle,
        command: &str,
        input: Value,
    ) -> Result<Value> {
        match command {
            "status" => settings(&app).await,
            "setup_check" => {
                if input["step_id"] != "notification-permission" {
                    bail!("invalid: unknown notification setup step");
                }
                let status = settings(&app).await?;
                let permission = status["permission"].as_str().unwrap_or("unknown");
                Ok(serde_json::to_value(PluginReadinessResult {
                    step_id: "notification-permission".into(),
                    state: if matches!(permission, "authorized" | "provisional" | "ephemeral") {
                        PluginSetupState::Ready
                    } else {
                        PluginSetupState::NeedsSetup
                    },
                    detail: format!(
                        "Notification authorization: {permission}. No permission prompt or notification was sent; alert settings may still limit presentation"
                    ),
                })?)
            }
            "request_permission" => {
                if input["approve"].as_bool() != Some(true) {
                    bail!("forbidden: explicitly request OS notification permission");
                }
                let attempt = attempt();
                begin_attempt(&attempt)?;
                let completion = attempt.clone();
                on_main(app.clone(),move|app| {
                    let app=app.clone();
                    let block=RcBlock::new(move|granted:Bool,error:*mut NSError| {
                        let code=if error.is_null(){None}else{Some(unsafe {&*error}.code())};
                        finish_attempt(&completion,granted.as_bool(),code);
                        let _=app.emit("notification-permission-changed",json!({"granted":granted.as_bool(),"failed":!error.is_null()}));
                    });
                    UNUserNotificationCenter::currentNotificationCenter().requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert|UNAuthorizationOptions::Sound|UNAuthorizationOptions::Badge,&block);
                    // The OS owns the prompt. Do not keep a broker command open
                    // while the user decides or claim that asking granted access.
                    Ok(json!({"requested":true,"permission":"pending","detail":"Check notification status after responding to the OS prompt"}))
                }).await.map_err(|error|{finish_attempt(&attempt,false,Some(-1));error})
            }
            "show" => {
                let status = settings(&app).await?;
                if !matches!(
                    status["permission"].as_str(),
                    Some("authorized" | "provisional" | "ephemeral")
                ) {
                    bail!(
                        "forbidden: enable notifications through the explicit permission request first"
                    );
                }
                let id = identifier(&input).map_err(anyhow::Error::msg)?;
                let title = input["title"]
                    .as_str()
                    .context("invalid: notification title required")?
                    .to_owned();
                let body = input["body"]
                    .as_str()
                    .context("invalid: notification body required")?
                    .to_owned();
                if title.trim().is_empty()
                    || title.len() > 120
                    || body.len() > 2000
                    || title.contains('\0')
                    || body.contains('\0')
                {
                    bail!("invalid: notification title/body exceeds limits");
                }
                let (send, recv) = tokio::sync::oneshot::channel();
                let send = Mutex::new(Some(send));
                on_main(app, move |_| {
                    let content = UNMutableNotificationContent::new();
                    content.setTitle(&NSString::from_str(&title));
                    content.setBody(&NSString::from_str(&body));
                    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                        &NSString::from_str(&id),
                        &content,
                        None,
                    );
                    let block = RcBlock::new(move |error: *mut NSError| {
                        if let Ok(mut send) = send.lock() {
                            if let Some(send) = send.take() {
                                let _ = send.send(error.is_null());
                            }
                        }
                    });
                    UNUserNotificationCenter::currentNotificationCenter()
                        .addNotificationRequest_withCompletionHandler(&request, Some(&block));
                    Ok(())
                })
                .await?;
                let accepted = tokio::time::timeout(std::time::Duration::from_secs(3), recv)
                    .await
                    .context("Notification submission outcome unknown")?
                    .context("Notification submission interrupted")?;
                if !accepted {
                    bail!("invalid: OS rejected notification submission");
                }
                Ok(
                    json!({"submitted":true,"detail":"Accepted by the OS; display may depend on Focus and notification settings"}),
                )
            }
            "dismiss" => {
                let id = identifier(&input).map_err(anyhow::Error::msg)?;
                on_main(app, move |_| {
                    let ids = NSArray::from_retained_slice(&[NSString::from_str(&id)]);
                    let center = UNUserNotificationCenter::currentNotificationCenter();
                    center.removePendingNotificationRequestsWithIdentifiers(&ids);
                    center.removeDeliveredNotificationsWithIdentifiers(&ids);
                    Ok(json!({"dismissed":true}))
                })
                .await
            }
            _ => bail!("unknown notification provider command"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permission_attempts_do_not_claim_success_or_duplicate_pending_requests() {
        let state = Arc::new(Mutex::new(None));
        begin_attempt(&state).unwrap();
        assert!(begin_attempt(&state).is_err());
        assert_eq!(state.lock().unwrap().as_ref().unwrap().state, "pending");
        finish_attempt(&state, false, Some(1));
        assert_eq!(state.lock().unwrap().as_ref().unwrap().state, "failed");
        begin_attempt(&state).unwrap();
        finish_attempt(&state, false, None);
        assert_eq!(state.lock().unwrap().as_ref().unwrap().state, "denied");
        begin_attempt(&state).unwrap();
        finish_attempt(&state, true, None);
        assert_eq!(state.lock().unwrap().as_ref().unwrap().state, "granted");
    }
}
