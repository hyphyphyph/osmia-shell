pub mod bar;
pub mod launcher;
pub mod panels;
pub mod popups;
pub mod settings;
pub mod widgets;

/// What a drawn frame asks the app to do. The app runs these after the frame.
#[derive(Debug, Clone)]
pub enum Action {
    /// Stop these processes (SIGTERM).
    KillProcs(Vec<u32>),
    /// Set (or, with no values, clear) the CPU and memory limit of a program.
    SetProcLimit(String, crate::limits::Limit),
    /// Hold the list order (with the names to hold), or release it.
    ProcLock(Option<Vec<String>>),
    ToggleOverlay(Overlay, f32),
    CloseOverlay,
    NotifClick(u32),
    NotifDismiss(u32),
    NotifButton(u32, String),
    ClearPopups,
    ClearHistory,
    HistoryRemove(u32),
    ToggleDnd,
    Power(PowerAction),
    VolumeScroll(i32),
    BrightnessScroll(i32),
    SetScreenTimeout(u32),
    SetBrightness(u32),
    SetPowerProfile(String),
    /// (dotted key such as "bar.height", value)
    SetConfig(String, ConfValue),
    OpenConfigFile,
    OpenSettings,
    /// Make this desktop file the default for category N of the Default applications page.
    SetDefaultApp(usize, String),
    CaptureCancel,
    CaptureOpen,
    /// (player bus name)
    MediaPlayPause(String),
    MediaNext(String),
    MediaPrev(String),
    MediaRaise(String),
    /// (player bus name, position in microseconds)
    MediaSeek(String, i64),
    CaptureGo(Target),
    StopRecording,
    SetLockTimeout(u32),
    ReloadConfig,
    ResetConfig,
    /// Switch to the workspace at this position in the list.
    GotoWorkspace(usize),
    /// Start or stop listening for the next key press, to name a shortcut.
    ShortcutRecord(bool),
    /// The recorded key was used; stop holding it.
    ShortcutTaken,
    ShortcutSetKey(usize, String),
    ShortcutSetCommand(usize, String),
    ShortcutDelete(usize),
    ShortcutAdd(String, String),
    ShortcutsRestore,
    ToggleMute,
    /// (is_input, device name, percent)
    AudioVolume(bool, String, u32),
    /// (is_input, device name)
    AudioMute(bool, String),
    /// (is_input, device name, port)
    AudioSelect(bool, String, Option<String>),
    FocusWindow(usize),
    NetConnect(String, bool),
    /// Join a network that needs a password: (name, password).
    NetJoin(String, String),
    NetRescan,
    TrayActivate(String),
    TrayMenuOpen(String, f32),
    TrayMenuClick(String, i32),
    TrayNativeMenu(String),
    RunApp(String),
    RunCommand(String),
    CopyAndClose(String),
}

/// What the user picked to capture. Indexes point into the capture state.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// Output index and the rectangle in that output's logical coordinates.
    Region(usize, egui::Rect),
    Screen(usize),
    Window(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    List(Vec<String>),
}

/// One open window, as the window menu lists it.
pub struct WinEntry {
    pub index: usize,
    pub title: String,
    pub app_id: String,
    pub app_name: String,
    pub minimized: bool,
    pub active: bool,
    pub alert: bool,
    pub ws: Option<String>,
}

pub struct WsEntry {
    pub index: usize,
    pub name: String,
    pub active: bool,
}

#[derive(Default)]
pub struct WinList {
    pub wins: Vec<WinEntry>,
    pub spaces: Vec<WsEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    History,
    Calendar(i32),
    Power(Option<PowerAction>),
    Launcher,
    TrayMenu(String),
    Network,
    Screen,
    Audio,
    Battery,
    Media,
    Windows,
    Cpu,
    Ram,
}

impl Overlay {
    pub fn same_kind(&self, other: &Overlay) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PowerAction {
    Lock,
    Logout,
    Suspend,
    Reboot,
    PowerOff,
}

impl PowerAction {
    pub fn label(self) -> &'static str {
        match self {
            PowerAction::Lock => "Lock",
            PowerAction::Logout => "Log out",
            PowerAction::Suspend => "Suspend",
            PowerAction::Reboot => "Reboot",
            PowerAction::PowerOff => "Power off",
        }
    }
    pub fn needs_confirm(self) -> bool {
        matches!(self, PowerAction::Logout | PowerAction::Reboot | PowerAction::PowerOff)
    }
}
