#![cfg(target_os = "windows")]

use anyhow::{Context, bail};
use std::ffi::OsStr;
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CreateMutexW};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, IDC_ARROW, IDI_APPLICATION, LoadCursorW,
    LoadIconW, MF_CHECKED, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow, SetTimer,
    TPM_LEFTALIGN, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
    WS_OVERLAPPED,
};

const TRAY_ID: u32 = 1;
const WM_TRAY: u32 = WM_APP + 1;
const TIMER_ID: usize = 1;
const ID_STATUS: usize = 2001;
const ID_TOGGLE: usize = 2002;
const ID_RESTART: usize = 2003;
const ID_AUTOSTART: usize = 2004;
const ID_EXIT: usize = 2005;

const RESTART_DELAY: Duration = Duration::from_secs(2);
const AUTOSTART_VALUE: &str = "Omniphony";
const LEGACY_AUTOSTART_VALUE: &str = "Spatial";
const AUTOSTART_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

struct AppState {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    /// User intent. When false, there must be no audio-engine child process.
    enabled: bool,
    quitting: bool,
    next_restart: Option<Instant>,
    restart_count: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            child: None,
            stdin: None,
            enabled: true,
            quitting: false,
            next_restart: None,
            restart_count: 0,
        }
    }
}

struct HandleGuard(*mut core::ffi::c_void);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

static STATE: OnceLock<Mutex<AppState>> = OnceLock::new();
static TASKBAR_CREATED: OnceLock<u32> = OnceLock::new();

fn state() -> &'static Mutex<AppState> {
    STATE.get_or_init(|| Mutex::new(AppState::default()))
}

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain(Some(0)).collect()
}

fn copy_wide<const N: usize>(target: &mut [u16; N], text: &str) {
    target.fill(0);
    let encoded: Vec<u16> = OsStr::new(text).encode_wide().collect();
    let count = encoded.len().min(N.saturating_sub(1));
    target[..count].copy_from_slice(&encoded[..count]);
}

fn taskbar_created_message() -> u32 {
    *TASKBAR_CREATED.get_or_init(|| {
        let name = wide("TaskbarCreated");
        unsafe { RegisterWindowMessageW(name.as_ptr()) }
    })
}

fn claim_single_instance() -> anyhow::Result<Option<HandleGuard>> {
    // Keep the long-lived mutex name so older private builds cannot run beside
    // the installed tray supervisor.
    let name = wide("Local\\OmniphonyForHeadphones.Singleton");
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        bail!("CreateMutexW failed for Omniphony single-instance guard");
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Ok(None);
    }
    Ok(Some(HandleGuard(handle)))
}

fn executable_root() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("failed to resolve Omniphony executable path")?;
    Ok(exe
        .parent()
        .context("Omniphony executable has no parent directory")?
        .to_path_buf())
}

fn settings_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Omniphony")
}

fn runtime_log_path() -> PathBuf {
    let root = settings_root();
    let _ = create_dir_all(&root);
    root.join("omniphony.log")
}

fn append_log(message: &str) {
    let path = runtime_log_path();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[supervisor] {message}");
    }
}

fn autostart_marker() -> PathBuf {
    settings_root().join("autostart.disabled")
}

fn autostart_preferred() -> bool {
    !autostart_marker().is_file()
}

fn delete_run_value(name: &str) {
    let mut command = Command::new("reg.exe");
    command
        .creation_flags(CREATE_NO_WINDOW)
        .arg("DELETE")
        .arg(AUTOSTART_KEY)
        .arg("/v")
        .arg(name)
        .arg("/f");
    let _ = command.status();
}

fn set_run_entry(enabled: bool) -> anyhow::Result<()> {
    if enabled {
        let exe = std::env::current_exe().context("failed to resolve Omniphony autostart executable")?;
        let value = format!("\"{}\"", exe.display());
        let mut command = Command::new("reg.exe");
        command
            .creation_flags(CREATE_NO_WINDOW)
            .arg("ADD")
            .arg(AUTOSTART_KEY)
            .arg("/v")
            .arg(AUTOSTART_VALUE)
            .arg("/t")
            .arg("REG_SZ")
            .arg("/d")
            .arg(value)
            .arg("/f");
        let status = command.status().context("failed to launch reg.exe")?;
        if !status.success() {
            bail!("reg.exe could not register Omniphony autostart");
        }
        delete_run_value(LEGACY_AUTOSTART_VALUE);
    } else {
        delete_run_value(AUTOSTART_VALUE);
        delete_run_value(LEGACY_AUTOSTART_VALUE);
    }
    Ok(())
}

fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    let marker = autostart_marker();
    if enabled {
        set_run_entry(true)?;
        let _ = std::fs::remove_file(marker);
    } else {
        let _ = set_run_entry(false);
        if let Some(parent) = marker.parent() {
            create_dir_all(parent).context("failed to create Omniphony settings directory")?;
        }
        std::fs::write(marker, b"disabled\n")
            .context("failed to persist disabled autostart preference")?;
    }
    Ok(())
}

fn ensure_autostart() {
    if autostart_preferred() {
        if let Err(err) = set_run_entry(true) {
            append_log(&format!("autostart registration failed: {err:#}"));
        }
    } else {
        let _ = set_run_entry(false);
    }
}

fn transport_state_path() -> Option<PathBuf> {
    std::env::var_os("PROGRAMDATA")
        .map(PathBuf::from)
        .map(|root| root.join("Omniphony").join("development-transport.txt"))
}

fn repair_transport_default() {
    let Some(state_path) = transport_state_path() else {
        append_log("PROGRAMDATA unavailable; transport default was not reasserted");
        return;
    };
    let Ok(transport) = std::fs::read_to_string(&state_path) else {
        append_log("transport state unavailable; leaving Windows default endpoint unchanged");
        return;
    };
    let needles: &[&str] = match transport.trim() {
        "steam-streaming-speakers" => &["Steam Streaming Speakers"],
        "omniphony-development-endpoint" => &["Omniphony", "Spatial"],
        other => {
            append_log(&format!("unknown transport state '{other}'; default endpoint unchanged"));
            return;
        }
    };

    let Ok(root) = executable_root() else {
        append_log("could not resolve install root for transport repair");
        return;
    };
    let helper = root.join("support").join("OmniphonyEndpointCtl.exe");
    if !helper.is_file() {
        append_log(&format!("transport helper missing: {}", helper.display()));
        return;
    }

    let mut command = Command::new(&helper);
    command
        .creation_flags(CREATE_NO_WINDOW)
        .arg("set-default-name");
    for needle in needles {
        command.arg(needle);
    }
    match command.output() {
        Ok(output) if output.status.success() => {
            append_log(&format!("reasserted Windows transport default: {}", needles.join(" / ")));
        }
        Ok(output) => {
            append_log(&format!(
                "transport default repair failed with exit code {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Err(err) => append_log(&format!("transport default repair could not launch: {err}")),
    }
}

fn spawn_worker() -> anyhow::Result<()> {
    {
        let app = state().lock().expect("Omniphony supervisor state poisoned");
        if !app.enabled || app.quitting || app.child.is_some() {
            return Ok(());
        }
    }

    // The installer chooses and records the hidden routing endpoint once. Every
    // engine start reasserts that choice before capture begins so boot/device
    // churn cannot silently bypass Omniphony and send a second dry stream to the
    // physical FiiO endpoint.
    repair_transport_default();

    let root = executable_root()?;
    let executable = std::env::current_exe().context("failed to resolve Omniphony engine executable")?;

    let log_path = runtime_log_path();
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open {}", log_path.display()))?;
    let log_err = log.try_clone().context("failed to clone Omniphony log handle")?;

    let mut child = Command::new(&executable)
        .current_dir(&root)
        .env("OMNIPHONY_INTERNAL_ENGINE", "1")
        .env("OMNIPHONY_PROFILE", "external")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .with_context(|| {
            format!(
                "failed to launch internal audio engine from {}",
                executable.display()
            )
        })?;
    let stdin = child.stdin.take().context("audio engine stdin was not piped")?;

    let mut app = state().lock().expect("Omniphony supervisor state poisoned");
    if !app.enabled || app.quitting {
        drop(app);
        let _ = child.kill();
        let _ = child.wait();
        return Ok(());
    }
    app.child = Some(child);
    app.stdin = Some(stdin);
    app.next_restart = None;
    append_log("audio engine started with Current model");
    Ok(())
}

fn stop_worker() {
    let (mut child, mut stdin) = {
        let mut app = state().lock().expect("Omniphony supervisor state poisoned");
        app.next_restart = None;
        (app.child.take(), app.stdin.take())
    };

    if let Some(stdin) = stdin.as_mut() {
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.flush();
    }

    if let Some(child) = child.as_mut() {
        for _ in 0..12 {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn schedule_restart(detail: &str) {
    append_log(detail);
    let mut app = state().lock().expect("Omniphony supervisor state poisoned");
    if app.enabled && !app.quitting {
        app.next_restart = Some(Instant::now() + RESTART_DELAY);
        app.restart_count = app.restart_count.saturating_add(1);
    } else {
        app.next_restart = None;
    }
}

fn set_enabled(hwnd: HWND, enabled: bool) {
    {
        let mut app = state().lock().expect("Omniphony supervisor state poisoned");
        app.enabled = enabled;
        app.next_restart = None;
    }

    if enabled {
        append_log("Omniphony ON requested");
        if let Err(err) = spawn_worker() {
            schedule_restart(&format!("audio engine start failed: {err:#}"));
        }
    } else {
        append_log("Omniphony OFF requested; stopping audio engine");
        stop_worker();
    }
    update_tray_tip(hwnd);
}

fn restart_worker(hwnd: HWND) {
    let enabled = state()
        .lock()
        .expect("Omniphony supervisor state poisoned")
        .enabled;
    if !enabled {
        update_tray_tip(hwnd);
        return;
    }

    {
        let mut app = state().lock().expect("Omniphony supervisor state poisoned");
        app.next_restart = None;
    }
    stop_worker();
    if let Err(err) = spawn_worker() {
        schedule_restart(&format!("audio engine restart failed: {err:#}"));
    }
    update_tray_tip(hwnd);
}

fn poll_worker(hwnd: HWND) {
    let mut retry = false;
    let mut exited: Option<String> = None;
    {
        let now = Instant::now();
        let mut app = state().lock().expect("Omniphony supervisor state poisoned");
        if let Some(child) = app.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exited = Some(format!("audio engine exited: {status}"));
                    app.child = None;
                    app.stdin = None;
                    if app.enabled && !app.quitting {
                        app.next_restart = Some(now + RESTART_DELAY);
                        app.restart_count = app.restart_count.saturating_add(1);
                    } else {
                        app.next_restart = None;
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    exited = Some(format!("audio engine status failed: {err}"));
                    app.child = None;
                    app.stdin = None;
                    if app.enabled && !app.quitting {
                        app.next_restart = Some(now + RESTART_DELAY);
                        app.restart_count = app.restart_count.saturating_add(1);
                    } else {
                        app.next_restart = None;
                    }
                }
            }
        } else if app.enabled
            && !app.quitting
            && app
                .next_restart
                .map(|deadline| now >= deadline)
                .unwrap_or(false)
        {
            app.next_restart = None;
            retry = true;
        }
    }

    if let Some(detail) = exited {
        append_log(&detail);
    }
    if retry {
        if let Err(err) = spawn_worker() {
            schedule_restart(&format!("automatic audio recovery failed: {err:#}"));
        }
    }
    update_tray_tip(hwnd);
}

fn tray_status() -> String {
    let app = state().lock().expect("Omniphony supervisor state poisoned");
    if app.quitting {
        "Omniphony - stopping".to_string()
    } else if !app.enabled {
        "Omniphony - OFF".to_string()
    } else if app.child.is_some() {
        "Omniphony - ON - Current model".to_string()
    } else {
        format!("Omniphony - recovering audio ({})", app.restart_count)
    }
}

fn add_tray_icon(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ID;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
    copy_wide(&mut data.szTip, &tray_status());
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data);
    }
}

fn update_tray_tip(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ID;
    data.uFlags = NIF_TIP;
    copy_wide(&mut data.szTip, &tray_status());
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
    }
}

fn remove_tray_icon(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = TRAY_ID;
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

fn append_menu_item(menu: *mut core::ffi::c_void, flags: u32, id: usize, text: &str) {
    let text = wide(text);
    unsafe {
        let _ = AppendMenuW(menu, flags, id, text.as_ptr());
    }
}

fn show_tray_menu(hwnd: HWND) {
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return;
    }

    let (running, enabled, restarts) = {
        let app = state().lock().expect("Omniphony supervisor state poisoned");
        (app.child.is_some(), app.enabled, app.restart_count)
    };
    let status = if !enabled {
        "Omniphony: OFF".to_string()
    } else if running {
        "Omniphony: ON | Current model".to_string()
    } else {
        format!("Omniphony: recovering ({restarts})")
    };

    append_menu_item(menu, MF_STRING | MF_GRAYED, ID_STATUS, &status);
    append_menu_item(
        menu,
        MF_STRING,
        ID_TOGGLE,
        if enabled {
            "Turn Omniphony off"
        } else {
            "Turn Omniphony on"
        },
    );
    append_menu_item(
        menu,
        MF_STRING | if enabled { 0 } else { MF_GRAYED },
        ID_RESTART,
        "Restart audio engine",
    );
    append_menu_item(
        menu,
        MF_STRING | if autostart_preferred() { MF_CHECKED } else { 0 },
        ID_AUTOSTART,
        "Start with Windows",
    );
    unsafe {
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    }
    append_menu_item(menu, MF_STRING, ID_EXIT, "Exit Omniphony");

    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        let _ = GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        let _ = PostMessageW(hwnd, WM_NULL, 0, 0);
        let _ = DestroyMenu(menu);
    }
}

fn shutdown(hwnd: HWND) {
    {
        let mut app = state().lock().expect("Omniphony supervisor state poisoned");
        app.quitting = true;
        app.enabled = false;
        app.next_restart = None;
    }
    remove_tray_icon(hwnd);
    stop_worker();
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == taskbar_created_message() {
        add_tray_icon(hwnd);
        update_tray_tip(hwnd);
        return 0;
    }

    match message {
        WM_CREATE => {
            add_tray_icon(hwnd);
            unsafe {
                SetTimer(hwnd, TIMER_ID, 500, None);
            }
            if let Err(err) = spawn_worker() {
                schedule_restart(&format!("initial audio engine start failed: {err:#}"));
                update_tray_tip(hwnd);
            }
            0
        }
        WM_TRAY => {
            let event = lparam as u32;
            if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                show_tray_menu(hwnd);
            }
            0
        }
        WM_COMMAND => {
            let command_id = wparam & 0xffff;
            match command_id {
                ID_TOGGLE => {
                    let enabled = state()
                        .lock()
                        .expect("Omniphony supervisor state poisoned")
                        .enabled;
                    set_enabled(hwnd, !enabled);
                }
                ID_RESTART => restart_worker(hwnd),
                ID_AUTOSTART => {
                    let desired = !autostart_preferred();
                    if let Err(err) = set_autostart(desired) {
                        append_log(&format!("could not change autostart preference: {err:#}"));
                    }
                }
                ID_EXIT => {
                    shutdown(hwnd);
                    unsafe {
                        DestroyWindow(hwnd);
                    }
                }
                _ => {}
            }
            0
        }
        WM_TIMER => {
            if wparam == TIMER_ID {
                poll_worker(hwnd);
            }
            0
        }
        WM_CLOSE => {
            shutdown(hwnd);
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

pub fn run() -> anyhow::Result<()> {
    let Some(_instance_guard) = claim_single_instance()? else {
        return Ok(());
    };
    ensure_autostart();
    let _ = taskbar_created_message();

    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        bail!("GetModuleHandleW failed");
    }

    let class_name = wide("OmniphonyAudioSupervisor");
    let title = wide("Omniphony");
    let mut class: WNDCLASSW = unsafe { std::mem::zeroed() };
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    class.lpszClassName = class_name.as_ptr();

    if unsafe { RegisterClassW(&class) } == 0 {
        bail!("RegisterClassW failed");
    }

    // Invisible top-level window: owns tray callbacks and watchdog timing only.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        bail!("CreateWindowExW failed");
    }

    let mut message: MSG = unsafe { std::mem::zeroed() };
    loop {
        let result = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
        if result == -1 {
            bail!("GetMessageW failed");
        }
        if result == 0 {
            break;
        }
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}
