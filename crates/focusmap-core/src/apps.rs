//! App categorisation.
//!
//! FocusMap never records window titles or document names - only the process
//! name of the foreground window. That is enough to know "this minute was spent
//! in an editor, a chat app or a game", which is what the scoring needs to stop
//! counting video and gaming as deep work.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppCategory {
    Editor,
    Terminal,
    Office,
    Browser,
    Chat,
    Meeting,
    Media,
    Game,
    #[default]
    Other,
}

impl AppCategory {
    pub fn label(self) -> &'static str {
        match self {
            AppCategory::Editor => "Editor",
            AppCategory::Terminal => "Terminal",
            AppCategory::Office => "Documents",
            AppCategory::Browser => "Browser",
            AppCategory::Chat => "Chat",
            AppCategory::Meeting => "Meeting",
            AppCategory::Media => "Media",
            AppCategory::Game => "Game",
            AppCategory::Other => "Other",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AppCategory::Editor => "editor",
            AppCategory::Terminal => "terminal",
            AppCategory::Office => "office",
            AppCategory::Browser => "browser",
            AppCategory::Chat => "chat",
            AppCategory::Meeting => "meeting",
            AppCategory::Media => "media",
            AppCategory::Game => "game",
            AppCategory::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "editor" => AppCategory::Editor,
            "terminal" => AppCategory::Terminal,
            "office" => AppCategory::Office,
            "browser" => AppCategory::Browser,
            "chat" => AppCategory::Chat,
            "meeting" => AppCategory::Meeting,
            "media" => AppCategory::Media,
            "game" => AppCategory::Game,
            _ => AppCategory::Other,
        }
    }

    /// Hard ceiling for the minute score. Watching a video or playing a game can
    /// produce plenty of input without being deep work, so those categories can
    /// never reach the deep band.
    pub fn score_cap(self) -> u8 {
        match self {
            AppCategory::Game | AppCategory::Media => 25,
            AppCategory::Chat => 50,
            AppCategory::Meeting => 45,
            _ => 100,
        }
    }

    pub fn is_passive(self) -> bool {
        matches!(self, AppCategory::Game | AppCategory::Media)
    }
}

/// Map an executable name (`Code.exe`, `chrome`, ...) to a category.
pub fn categorize(exe: &str) -> AppCategory {
    let name = exe
        .trim()
        .trim_end_matches(".exe")
        .trim_end_matches(".EXE")
        .to_ascii_lowercase();
    if name.is_empty() {
        return AppCategory::Other;
    }

    // Meeting apps before chat: Teams and Zoom belong in both worlds, and a
    // meeting is the weaker focus signal of the two.
    for m in [
        "zoom", "teams", "ms-teams", "webex", "whereby", "gotomeeting", "bluejeans",
    ] {
        if name == m {
            return AppCategory::Meeting;
        }
    }
    for m in [
        "discord",
        "slack",
        "telegram",
        "whatsapp",
        "signal",
        "skype",
        "messenger",
        "element",
        "mattermost",
        "rocket.chat",
    ] {
        if name == m {
            return AppCategory::Chat;
        }
    }
    for m in [
        "vlc", "mpv", "spotify", "netflix", "potplayermini64", "potplayer", "musicbee", "itunes",
        "twitch", "primevideo", "disneyplus", "wmplayer", "foobar2000",
    ] {
        if name == m {
            return AppCategory::Media;
        }
    }
    for m in [
        "steam",
        "steamwebhelper",
        "epicgameslauncher",
        "battle.net",
        "riotclient",
        "valorant",
        "dota2",
        "cs2",
        "minecraft",
        "javaw",
        "gta5",
        "eldenring",
        "leagueclient",
        "overwatch",
    ] {
        if name == m {
            return AppCategory::Game;
        }
    }
    for m in [
        "windowsterminal",
        "wt",
        "powershell",
        "pwsh",
        "cmd",
        "conhost",
        "alacritty",
        "wezterm",
        "mintty",
        "tabby",
        "kitty",
        "wsl",
    ] {
        if name == m {
            return AppCategory::Terminal;
        }
    }
    for m in [
        "code",
        "code-insiders",
        "cursor",
        "windsurf",
        "zed",
        "sublime_text",
        "notepad++",
        "idea64",
        "pycharm64",
        "clion64",
        "studio64",
        "rustrover64",
        "goland64",
        "webstorm64",
        "devenv",
        "nvim",
        "gvim",
        "vim",
        "emacs",
        "helix",
        "fleet",
        "xed",
    ] {
        if name == m {
            return AppCategory::Editor;
        }
    }
    for m in [
        "winword",
        "excel",
        "powerpnt",
        "onenote",
        "notepad",
        "obsidian",
        "notion",
        "typora",
        "acrobat",
        "acrord32",
        "sumatrapdf",
        "xournalpp",
        "writer",
        "soffice",
    ] {
        if name == m {
            return AppCategory::Office;
        }
    }
    for m in [
        "chrome", "msedge", "firefox", "brave", "opera", "vivaldi", "arc", "chromium", "librewolf",
        "zen",
    ] {
        if name == m {
            return AppCategory::Browser;
        }
    }
    AppCategory::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorizes_common_apps() {
        assert_eq!(categorize("Code.exe"), AppCategory::Editor);
        assert_eq!(categorize("chrome.EXE"), AppCategory::Browser);
        assert_eq!(categorize("Teams"), AppCategory::Meeting);
        assert_eq!(categorize("Discord.exe"), AppCategory::Chat);
        assert_eq!(categorize("VLCC"), AppCategory::Other);
        assert_eq!(categorize("vlc"), AppCategory::Media);
        assert_eq!(categorize("steam.exe"), AppCategory::Game);
        assert_eq!(categorize(""), AppCategory::Other);
    }

    #[test]
    fn passive_categories_are_capped() {
        assert!(AppCategory::Media.score_cap() < 70);
        assert!(AppCategory::Game.score_cap() < 70);
        assert_eq!(AppCategory::Editor.score_cap(), 100);
    }
}
