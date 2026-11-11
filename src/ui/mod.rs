pub mod bar;
pub mod clipboard;
pub mod emoji;
pub mod launcher;
pub mod osd;
pub mod overview;
pub mod panels;
pub mod popdown;
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
    /// Set the keyboard backlight to this level.
    SetKbdLight(u32),
    SetPowerProfile(String),
    /// (dotted key such as "bar.height", value)
    SetConfig(String, ConfValue),
    OpenConfigFile,
    OpenSettings,
    /// Make this desktop file the default for category N of the Default applications page.
    SetDefaultApp(usize, String),
    /// Choose the file picker backend by its portal name. None is Automatic.
    SetFilePicker(Option<String>),
    /// Ask the portal for a file dialog, to see the picker.
    TryFilePicker,
    /// Make GTK apps use the portal's file dialog.
    SetGtkPortal(bool),
    /// Install the picker with this portal name, in a terminal.
    InstallPicker(String),
    CaptureCancel,
    CaptureOpen,
    /// (player bus name)
    MediaPlayPause(String),
    MediaNext(String),
    MediaPrev(String),
    MediaRaise(String),
    /// Like (true) or unlike a Spotify track, by its spotify:track: URI.
    MediaLike(String, bool),
    /// Ask Spotify whether a track is liked, unless already known.
    MediaLikeCheck(String),
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
    /// Replace a shortcut's actions.
    ShortcutSetSteps(usize, Vec<crate::keybinds::Step>),
    /// Replace a shortcut's actions with XML typed by hand.
    ShortcutSetXml(usize, String),
    ShortcutDelete(usize),
    ShortcutAdd(String, Vec<crate::keybinds::Step>),
    ShortcutsRestore,
    ToggleMute,
    /// (is_input, device name, percent)
    AudioVolume(bool, String, u32),
    /// (is_input, device name)
    AudioMute(bool, String),
    /// (is_input, device name, port)
    AudioSelect(bool, String, Option<String>),
    FocusWindow(usize),
    /// Ask a window to close, by its index in the window list.
    CloseWindow(usize),
    NetConnect(String, bool),
    /// Join a network that needs a password: (name, password).
    NetJoin(String, String),
    NetRescan,
    TrayActivate(String),
    TrayMenuOpen(String, f32),
    TrayMenuClick(String, i32),
    TrayNativeMenu(String),
    /// Quit the app behind a tray item.
    TrayQuit(String),
    RunApp(String),
    RunCommand(String),
    CopyAndClose(String),
    /// Copy this entry of the clipboard history again and close.
    ClipPick(usize),
    ClipDelete(usize),
    ClipClear,
    /// Copy this emoji (an index into the emoji list) in the chosen skin tone and close.
    EmojiPick(usize),
    /// Choose the skin tone, 0 to 4, or None for plain.
    EmojiTone(Option<usize>),
    /// Ask the graphics card service for something.
    Gpu(crate::gpu::Cmd),
    /// Show or hide the second click that confirms Force off.
    GpuConfirmForce(bool),
    /// Ask BlueZ for something.
    Bt(crate::bt::Cmd),
    /// Answer what the pairing agent asked, by request id.
    BtAnswer(u64, crate::bt::Answer),
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
    Clipboard,
    Emoji,
    TrayMenu(String),
    Network,
    Screen,
    Audio,
    Battery,
    Media,
    Windows,
    Cpu,
    Ram,
    Gpu,
    Bluetooth,
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
    Hibernate,
    Reboot,
    PowerOff,
}

impl PowerAction {
    pub fn label(self) -> &'static str {
        match self {
            PowerAction::Lock => "Lock",
            PowerAction::Logout => "Log out",
            PowerAction::Suspend => "Suspend",
            PowerAction::Hibernate => "Hibernate",
            PowerAction::Reboot => "Reboot",
            PowerAction::PowerOff => "Power off",
        }
    }
    pub fn needs_confirm(self) -> bool {
        matches!(self, PowerAction::Logout | PowerAction::Reboot | PowerAction::PowerOff)
    }
}
