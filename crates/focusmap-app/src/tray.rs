//! System tray: pause, open, mini window, quit.
//!
//! The icon is drawn in code so the binary ships no image assets.

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub enum TrayAction {
    Open,
    TogglePause,
    Mini,
    Quit,
}

pub struct TrayHandle {
    tray: TrayIcon,
    open: MenuItem,
    pause: MenuItem,
    mini: MenuItem,
    quit: MenuItem,
    tooltip: String,
}

impl TrayHandle {
    pub fn new(paused: bool) -> Option<Self> {
        let menu = Menu::new();
        let open = MenuItem::new("Open FocusMap", true, None);
        let pause = MenuItem::new(pause_label(paused), true, None);
        let mini = MenuItem::new("Mini window", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let separator = PredefinedMenuItem::separator();
        menu.append_items(&[&open, &pause, &mini, &separator, &quit])
            .ok()?;
        let tray = TrayIconBuilder::new()
            .with_id("focusmap-tray")
            .with_tooltip("FocusMap")
            .with_menu(Box::new(menu))
            .with_icon(icon())
            .build()
            .ok()?;
        Some(Self {
            tray,
            open,
            pause,
            mini,
            quit,
            tooltip: "FocusMap".into(),
        })
    }

    pub fn poll(&self) -> Vec<TrayAction> {
        let mut actions = Vec::new();
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == *self.open.id() {
                actions.push(TrayAction::Open);
            } else if event.id == *self.pause.id() {
                actions.push(TrayAction::TogglePause);
            } else if event.id == *self.mini.id() {
                actions.push(TrayAction::Mini);
            } else if event.id == *self.quit.id() {
                actions.push(TrayAction::Quit);
            }
        }
        actions
    }

    pub fn set_paused(&self, paused: bool) {
        self.pause.set_text(pause_label(paused));
    }

    /// Cheap no-op when the text did not change, so the shell is not spammed.
    pub fn set_tooltip(&mut self, text: &str) {
        if text != self.tooltip {
            let _ = self.tray.set_tooltip(Some(text));
            self.tooltip = text.to_string();
        }
    }
}

fn pause_label(paused: bool) -> &'static str {
    if paused {
        "Resume tracking"
    } else {
        "Pause tracking"
    }
}

/// A 32 px icon: purple disc with a white focus ring.
pub fn icon() -> Icon {
    let (rgba, width, height) = icon_rgba();
    Icon::from_rgba(rgba, width, height).unwrap_or_else(|_| {
        Icon::from_rgba(vec![123, 108, 255, 255], 1, 1).expect("1x1 icon")
    })
}

fn icon_rgba() -> (Vec<u8>, u32, u32) {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let center = 15.5f32;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let idx = ((y * size + x) * 4) as usize;
            if distance <= 15.2 {
                rgba[idx] = 0x7B;
                rgba[idx + 1] = 0x6C;
                rgba[idx + 2] = 0xFF;
                rgba[idx + 3] = 255;
            }
            if (distance - 8.2).abs() <= 1.8 {
                let angle = dy.atan2(dx).to_degrees();
                let normalized = (angle + 90.0 + 360.0) % 360.0;
                if normalized <= 268.0 {
                    rgba[idx] = 255;
                    rgba[idx + 1] = 255;
                    rgba[idx + 2] = 255;
                    rgba[idx + 3] = 255;
                }
            }
        }
    }
    (rgba, size, size)
}

pub fn window_icon() -> egui::IconData {
    let (rgba, width, height) = icon_rgba();
    egui::IconData {
        rgba,
        width,
        height,
    }
}
