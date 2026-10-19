//! Napojenie na Windows pre zákazky CV (`.cvj`), iba pre prihláseného používateľa (HKCU):
//! položka „Otvoriť v CabinetCRAFT“ v ponuke pravého tlačidla a program v zozname
//! „Otvoriť pomocou“ (odtiaľ si ho používateľ môže nastaviť ako predvolený). Predvolený
//! program pre `.cvj` (CABINET VISION) sa nemení. Spúšťa sa iba na klik používateľa.

/// Položka ponuky pre `.cvj` (nezávislá od programu, ktorý `.cvj` otvára).
#[cfg(windows)]
const MENU_KEY: &str = r"HKCU\Software\Classes\SystemFileAssociations\.cvj\shell\CabinetCRAFT";
/// Program v zozname „Otvoriť pomocou“.
#[cfg(windows)]
const APP_KEY: &str = r"HKCU\Software\Classes\Applications\cabinetcraft-app.exe";

#[cfg(windows)]
fn reg(args: &[&str]) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // bez okna konzoly
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| cc_i18n::tr!("shell.reg_failed", error = e.to_string()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// Je položka „Otvoriť v CabinetCRAFT“ pre `.cvj` zapísaná?
#[cfg(windows)]
pub fn registered() -> bool {
    reg(&["query", &format!(r"{MENU_KEY}\command")]).is_ok()
}

#[cfg(not(windows))]
pub fn registered() -> bool {
    false
}

/// Zapíše položku ponuky a program do „Otvoriť pomocou“ pre tento `.exe`.
#[cfg(windows)]
pub fn register() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| cc_i18n::tr!("shell.exe_path", error = e.to_string()))?;
    let exe = exe.display().to_string();
    let command = format!("\"{exe}\" \"%1\"");
    let icon = format!("\"{exe}\",0");
    // text položky v jazyku okna v čase zápisu
    let item = cc_i18n::tr!("shell.menu_item");
    reg(&["add", MENU_KEY, "/ve", "/d", &item, "/f"])?;
    reg(&["add", MENU_KEY, "/v", "Icon", "/d", &icon, "/f"])?;
    reg(&[
        "add",
        &format!(r"{MENU_KEY}\command"),
        "/ve",
        "/d",
        &command,
        "/f",
    ])?;
    reg(&[
        "add",
        APP_KEY,
        "/v",
        "FriendlyAppName",
        "/d",
        "CabinetCRAFT",
        "/f",
    ])?;
    reg(&[
        "add",
        &format!(r"{APP_KEY}\SupportedTypes"),
        "/v",
        ".cvj",
        "/d",
        "",
        "/f",
    ])?;
    reg(&[
        "add",
        &format!(r"{APP_KEY}\shell\open\command"),
        "/ve",
        "/d",
        &command,
        "/f",
    ])?;
    Ok(())
}

#[cfg(not(windows))]
pub fn register() -> Result<(), String> {
    Err(cc_i18n::tr!("shell.windows_only"))
}

/// Odstráni položku ponuky a program zo zoznamu „Otvoriť pomocou“.
#[cfg(windows)]
pub fn unregister() -> Result<(), String> {
    let menu = reg(&["delete", MENU_KEY, "/f"]);
    let app = reg(&["delete", APP_KEY, "/f"]);
    menu.and(app)
}

#[cfg(not(windows))]
pub fn unregister() -> Result<(), String> {
    Err(cc_i18n::tr!("shell.windows_only"))
}
