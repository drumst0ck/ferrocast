//! Custom themes: selecting, editing, duplicating, renaming, deleting, importing and exporting.
//! The pure parts (model, validation, contrast, file format) live in `ferrocast_core::themes`.

use super::*;
use ferrocast_core::themes::{self, CustomTheme, ThemeBase};

/// Largest theme file Import reads (a real one is under 1 KB).
const MAX_THEME_FILE: u64 = 64 * 1024;

impl App {
    /// Use the custom theme called `name` (`None` goes back to the System / Dark / Light setting).
    pub(crate) fn select_theme(&mut self, name: Option<String>) {
        let mut s = self.settings.clone();
        s.custom_theme = name;
        self.commit_settings(s);
    }

    /// Store an edited version of the selected theme; colours repaint at once.
    pub(crate) fn update_active_theme(&mut self, edited: CustomTheme) {
        let Some(name) = self.settings.custom_theme.clone() else { return };
        let Some(slot) = self.custom_themes.iter_mut().find(|t| t.name.eq_ignore_ascii_case(&name)) else { return };
        let base_changed = slot.base != edited.base;
        if *slot == edited {
            return;
        }
        *slot = edited;
        self.save_themes();
        if base_changed {
            self.set_theme();
        }
    }

    /// An editable copy of the selected theme, or of the built-in one in effect, selected at once.
    pub(crate) fn duplicate_theme(&mut self) {
        let source = match self.active_theme() {
            Some(t) => t.clone(),
            None => themes::builtin(if self.dark { ThemeBase::Dark } else { ThemeBase::Light }),
        };
        let copy = source.duplicate_of(&self.custom_themes);
        let name = copy.name.clone();
        self.custom_themes.push(copy);
        self.save_themes();
        self.select_theme(Some(name));
    }

    /// Remove the selected theme; the built-in setting applies again.
    pub(crate) fn delete_active_theme(&mut self) {
        let Some(name) = self.settings.custom_theme.clone() else { return };
        self.custom_themes.retain(|t| !t.name.eq_ignore_ascii_case(&name));
        self.save_themes();
        self.select_theme(None);
    }

    /// Rename the selected theme; the name must be 1-60 characters and not used by another theme.
    pub(crate) fn rename_active_theme(&mut self, new: &str) -> Result<(), String> {
        let Some(old) = self.settings.custom_theme.clone() else { return Err("No custom theme selected".into()) };
        let new = new.trim();
        let Some(i) = self.custom_themes.iter().position(|t| t.name.eq_ignore_ascii_case(&old)) else {
            return Err("Theme not found".into());
        };
        let mut renamed = self.custom_themes[i].clone();
        renamed.name = new.to_string();
        renamed.validate().map_err(|e| e.to_string())?;
        if self.custom_themes.iter().enumerate().any(|(j, t)| j != i && t.name.eq_ignore_ascii_case(new)) {
            return Err(format!("A theme called \"{new}\" already exists"));
        }
        self.custom_themes[i] = renamed;
        self.save_themes();
        self.select_theme(Some(new.to_string()));
        Ok(())
    }

    /// Export…: ask where in the background; `theme_file_picked` writes the file.
    pub(crate) fn export_theme_dialog(&mut self) {
        let Some(t) = self.active_theme() else { return };
        let name = t.name.clone();
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            if let Some(path) = services::choose_theme_export_path(&name) {
                bus.send(Msg::ThemePath { export: true, path });
            }
        });
    }

    /// Import…: ask which file in the background; `theme_file_picked` reads it.
    pub(crate) fn import_theme_dialog(&mut self) {
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            if let Some(path) = services::choose_theme_import_path() {
                bus.send(Msg::ThemePath { export: false, path });
            }
        });
    }

    pub(crate) fn theme_file_picked(&mut self, export: bool, path: PathBuf) {
        if export {
            let Some(t) = self.active_theme().cloned() else { return };
            match std::fs::write(&path, themes::export_json(&t)) {
                Ok(()) => self.toast(format!("Exported \"{}\"", t.name)),
                Err(e) => self.toast(format!("Could not export the theme: {e}")),
            }
        } else {
            match self.import_theme_file(&path) {
                Ok(name) => self.toast(format!("Imported \"{name}\"")),
                Err(e) => self.toast(e),
            }
        }
    }

    /// Read a `.ferrocast-theme` file, save it under a free name and select it.
    pub(crate) fn import_theme_file(&mut self, path: &std::path::Path) -> Result<String, String> {
        let meta = std::fs::metadata(path).map_err(|e| format!("Could not read the file: {e}"))?;
        if meta.len() > MAX_THEME_FILE {
            return Err("That file is too large to be a theme".into());
        }
        let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read the file: {e}"))?;
        let mut t = themes::import_json(&text).map_err(|e| e.to_string())?;
        t.name = themes::unique_name(&self.custom_themes, &t.name);
        let name = t.name.clone();
        self.custom_themes.push(t);
        self.save_themes();
        self.select_theme(Some(name.clone()));
        Ok(name)
    }
}
