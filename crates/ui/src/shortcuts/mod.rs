//! Keyboard shortcuts: chords, a registry of command bindings, conflict detection and rebinding.
//!
//! A [`Chord`] is written portably: `Mod` is the platform's primary modifier (Cmd on macOS,
//! Ctrl elsewhere), so `Mod+S` is Save everywhere. `Ctrl` is the literal Control key, which is a
//! different key from Cmd only on macOS. Chords parse from and print to that portable form
//! (`Mod+Shift+Z`), and display in each platform's own style (`⇧⌘Z` on macOS,
//! `Ctrl+Shift+Z` elsewhere).
//!
//! The [`ShortcutRegistry`] holds each command's default chords and the user's overrides,
//! reports conflicts as the current platform sees them (`Mod+K` and `Ctrl+K` are the same
//! keys on Windows but not on macOS), and consumes matching key presses from egui's input,
//! most specific first, so `Mod+Shift+S` never also fires `Mod+S`.
//!
//! The registry knows nothing about what commands do: the app maps the returned
//! [`CommandId`]s to its own actions.

use std::collections::BTreeMap;
use std::fmt;

use egui::{InputState, Key, KeyboardShortcut, Modifiers};

#[cfg(test)]
mod tests;

/// The platform whose keyboard conventions apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    /// macOS and iOS: Cmd is primary, Control is a separate modifier, symbols in menus.
    Mac,
    /// Windows, Linux and everything else: Ctrl is primary.
    Other,
}

impl Platform {
    /// The platform this binary was built for. On the web, prefer [`Platform::from_egui`],
    /// which looks at the browser's user agent.
    pub const fn native() -> Self {
        if cfg!(any(target_os = "macos", target_os = "ios")) { Self::Mac } else { Self::Other }
    }

    /// The platform egui detected (`ctx.os()`), which also works in a browser.
    pub fn from_egui(os: egui::os::OperatingSystem) -> Self {
        match os {
            egui::os::OperatingSystem::Mac | egui::os::OperatingSystem::IOS => Self::Mac,
            egui::os::OperatingSystem::Unknown => Self::native(),
            _ => Self::Other,
        }
    }
}

/// One key with modifiers, such as `Mod+Shift+Z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    /// The primary modifier: Cmd on macOS, Ctrl elsewhere.
    pub primary: bool,
    /// The literal Control key. Only distinct from `primary` on macOS.
    pub ctrl: bool,
    /// Alt (Option on macOS).
    pub alt: bool,
    pub shift: bool,
    pub key: KeyName,
}

/// An egui key that can be ordered and hashed in collections.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyName(pub Key);

impl PartialOrd for KeyName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for KeyName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.name().cmp(other.0.name())
    }
}

/// Why a chord could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChordError {
    /// Nothing but whitespace.
    Empty,
    /// A part is not a modifier or key name egui knows.
    UnknownKey(String),
    /// More than one non-modifier key, such as `A+B`.
    TwoKeys,
    /// Modifiers without a key, such as `Mod+Shift`.
    NoKey,
}

impl fmt::Display for ChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("the shortcut is empty"),
            Self::UnknownKey(k) => write!(f, "'{k}' is not a key name"),
            Self::TwoKeys => f.write_str("a shortcut has exactly one key besides the modifiers"),
            Self::NoKey => f.write_str("the shortcut has modifiers but no key"),
        }
    }
}

impl std::error::Error for ChordError {}

impl Chord {
    /// A chord with no modifiers.
    pub const fn key(key: Key) -> Self {
        Self { primary: false, ctrl: false, alt: false, shift: false, key: KeyName(key) }
    }

    /// `Mod+key`: Cmd on macOS, Ctrl elsewhere.
    pub const fn primary(key: Key) -> Self {
        Self { primary: true, ..Self::key(key) }
    }

    /// Adds Shift.
    pub const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// Adds Alt (Option).
    pub const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Adds the literal Control key.
    pub const fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    /// Reads `Mod+Shift+Z`, `Cmd+K`, `Ctrl+Alt+Delete`, `F5`, `Mod++` and the like.
    ///
    /// `Mod`, `Cmd`, `Command`, `Super`, `Meta` and `Primary` all mean the primary modifier;
    /// `Ctrl`/`Control`, `Alt`/`Option`/`Opt` and `Shift` the others. Case and spaces around `+`
    /// are ignored. Key names are egui's (`A`, `F5`, `Enter`, `ArrowUp`, `Comma`, `,`, …).
    pub fn parse(text: &str) -> Result<Self, ChordError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(ChordError::Empty);
        }
        // A trailing "++" means the key itself is "+".
        let (mods, trailing_plus) = match text.strip_suffix("++") {
            Some(rest) => (rest, true),
            None if text == "+" => ("", true),
            None => (text, false),
        };
        let mut chord = Self::key(Key::A);
        let mut key = trailing_plus.then_some(Key::Plus);
        for part in mods.split('+').map(str::trim).filter(|p| !p.is_empty()) {
            match part.to_ascii_lowercase().as_str() {
                "mod" | "cmd" | "command" | "super" | "meta" | "primary" | "⌘" => chord.primary = true,
                "ctrl" | "control" | "⌃" => chord.ctrl = true,
                "alt" | "option" | "opt" | "⌥" => chord.alt = true,
                "shift" | "⇧" => chord.shift = true,
                _ => {
                    let found = Key::from_name(part)
                        .or_else(|| Key::from_name(&part.to_ascii_uppercase()))
                        .or_else(|| Key::from_name(&title_case(part)))
                        .ok_or_else(|| ChordError::UnknownKey(part.to_owned()))?;
                    if key.replace(found).is_some() {
                        return Err(ChordError::TwoKeys);
                    }
                }
            }
        }
        chord.key = KeyName(key.ok_or(ChordError::NoKey)?);
        Ok(chord)
    }

    /// The portable text form, `Mod+Ctrl+Alt+Shift+Key`, which [`Chord::parse`] reads back.
    pub fn portable(&self) -> String {
        let mut out = String::new();
        for (on, name) in [(self.primary, "Mod"), (self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift")] {
            if on {
                out.push_str(name);
                out.push('+');
            }
        }
        out.push_str(self.key.0.name());
        out
    }

    /// The chord as menus on `platform` show it: `⌃⌥⇧⌘Z` on macOS, `Ctrl+Alt+Shift+Z` elsewhere.
    pub fn display(&self, platform: Platform) -> String {
        let key = self.key.0.symbol_or_name();
        match platform {
            Platform::Mac => {
                let mut out = String::new();
                for (on, symbol) in [(self.ctrl, "⌃"), (self.alt, "⌥"), (self.shift, "⇧"), (self.primary, "⌘")] {
                    if on {
                        out.push_str(symbol);
                    }
                }
                out.push_str(key);
                out
            }
            Platform::Other => {
                let mut out = String::new();
                for (on, name) in [(self.primary || self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift")] {
                    if on {
                        out.push_str(name);
                        out.push('+');
                    }
                }
                out.push_str(key);
                out
            }
        }
    }

    /// The keys actually pressed on `platform`. On Windows and Linux `Mod` and `Ctrl` are the same
    /// key, so `Mod+K` and `Ctrl+K` resolve to the same keys there.
    pub fn resolve(&self, platform: Platform) -> ResolvedChord {
        match platform {
            Platform::Mac => ResolvedChord { cmd: self.primary, ctrl: self.ctrl, alt: self.alt, shift: self.shift, key: self.key },
            Platform::Other => ResolvedChord { cmd: false, ctrl: self.primary || self.ctrl, alt: self.alt, shift: self.shift, key: self.key },
        }
    }

    /// The egui shortcut for `platform`, for `InputState::consume_shortcut` and menu buttons.
    pub fn to_egui(&self, platform: Platform) -> KeyboardShortcut {
        let r = self.resolve(platform);
        let modifiers = match platform {
            Platform::Mac => Modifiers { alt: r.alt, ctrl: r.ctrl, shift: r.shift, mac_cmd: r.cmd, command: r.cmd },
            Platform::Other => Modifiers { alt: r.alt, ctrl: r.ctrl, shift: r.shift, mac_cmd: false, command: r.ctrl },
        };
        KeyboardShortcut::new(modifiers, self.key.0)
    }

    /// How many modifiers the chord holds on `platform`, for matching the most specific first.
    fn specificity(&self, platform: Platform) -> usize {
        let r = self.resolve(platform);
        [r.cmd, r.ctrl, r.alt, r.shift].into_iter().filter(|on| *on).count()
    }
}

impl fmt::Display for Chord {
    /// The portable form.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.portable())
    }
}

impl std::str::FromStr for Chord {
    type Err = ChordError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A chord as the keys physically pressed on one platform; two chords conflict when these match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedChord {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: KeyName,
}

/// Names a command, such as `file.save` or `view.zoom-in`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(String);

impl CommandId {
    /// A command id: 1 to 128 ASCII letters, digits, `.`, `-` or `_`.
    pub fn new(id: impl Into<String>) -> Result<Self, RegistryError> {
        let id = id.into();
        let valid = !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
        if valid { Ok(Self(id)) } else { Err(RegistryError::BadId(id)) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A command the registry knows, with what the palette and menus show for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandInfo {
    pub id: CommandId,
    /// Shown in the palette and menus, such as `Save As…`.
    pub label: String,
    /// Groups commands in the palette, such as `File`.
    pub category: String,
    defaults: Vec<Chord>,
}

impl CommandInfo {
    /// The chords the app ships for this command.
    pub fn defaults(&self) -> &[Chord] {
        &self.defaults
    }
}

/// Two commands bound to the same keys on the current platform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// The keys, as the first command's chord.
    pub chord: Chord,
    pub first: CommandId,
    pub second: CommandId,
}

/// What the registry refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// The id is empty, too long or has characters other than letters, digits, `.`, `-`, `_`.
    BadId(String),
    /// A command with this id is already registered.
    Duplicate(CommandId),
    /// No command with this id is registered.
    Unknown(CommandId),
    /// The chord is already bound to another command on this platform.
    Taken { chord: Chord, by: CommandId },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadId(id) => write!(f, "'{id}' is not a valid command id"),
            Self::Duplicate(id) => write!(f, "the command '{id}' is registered twice"),
            Self::Unknown(id) => write!(f, "there is no command '{id}'"),
            Self::Taken { chord, by } => write!(f, "{chord} is already used by '{by}'"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// A problem found while applying saved overrides; the rest still applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverrideProblem {
    /// The saved id names no registered command (perhaps from another app version).
    UnknownCommand(String),
    /// A saved chord could not be read.
    BadChord { command: CommandId, text: String, error: ChordError },
}

/// The commands, their default chords and the user's rebindings.
#[derive(Clone, Debug)]
pub struct ShortcutRegistry {
    platform: Platform,
    commands: Vec<CommandInfo>,
    overrides: BTreeMap<CommandId, Vec<Chord>>,
}

impl Default for ShortcutRegistry {
    fn default() -> Self {
        Self::new(Platform::native())
    }
}

impl ShortcutRegistry {
    /// An empty registry for `platform`.
    pub fn new(platform: Platform) -> Self {
        Self { platform, commands: Vec::new(), overrides: BTreeMap::new() }
    }

    /// The platform conflicts and matching are judged for.
    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// Switches platform (for example once `ctx.os()` is known on the web).
    pub fn set_platform(&mut self, platform: Platform) {
        self.platform = platform;
    }

    /// Registers a command with its default chords (possibly none).
    ///
    /// Defaults that conflict with each other are allowed here, since apps may ship platform
    /// variants; [`ShortcutRegistry::conflicts`] reports them.
    pub fn register(
        &mut self,
        id: &str,
        label: impl Into<String>,
        category: impl Into<String>,
        defaults_in: &[Chord],
    ) -> Result<CommandId, RegistryError> {
        let id = CommandId::new(id)?;
        if self.info(&id).is_some() {
            return Err(RegistryError::Duplicate(id));
        }
        let mut defaults: Vec<Chord> = Vec::with_capacity(defaults_in.len());
        for chord in defaults_in {
            if !defaults.contains(chord) {
                defaults.push(*chord);
            }
        }
        self.commands.push(CommandInfo { id: id.clone(), label: label.into(), category: category.into(), defaults });
        Ok(id)
    }

    /// Every registered command, in registration order.
    pub fn commands(&self) -> &[CommandInfo] {
        &self.commands
    }

    /// The command registered as `id`.
    pub fn info(&self, id: &CommandId) -> Option<&CommandInfo> {
        self.commands.iter().find(|c| &c.id == id)
    }

    /// Looks a command up by its id text.
    pub fn find(&self, id: &str) -> Option<&CommandInfo> {
        self.commands.iter().find(|c| c.id.as_str() == id)
    }

    /// The chords in effect for `id`: the user's override if there is one, otherwise the defaults.
    pub fn bindings(&self, id: &CommandId) -> &[Chord] {
        match self.overrides.get(id) {
            Some(chords) => chords,
            None => self.info(id).map_or(&[], |c| c.defaults.as_slice()),
        }
    }

    /// The first chord in effect for `id`, formatted for this platform, for menus and the palette.
    pub fn display(&self, id: &CommandId) -> Option<String> {
        self.bindings(id).first().map(|c| c.display(self.platform))
    }

    /// Whether the user changed `id`'s chords.
    pub fn is_overridden(&self, id: &CommandId) -> bool {
        self.overrides.contains_key(id)
    }

    /// The command `chord` triggers on this platform, if any.
    pub fn command_for(&self, chord: Chord) -> Option<&CommandId> {
        let wanted = chord.resolve(self.platform);
        self.commands.iter().map(|c| &c.id).find(|id| self.bindings(id).iter().any(|c| c.resolve(self.platform) == wanted))
    }

    /// Binds `id` to exactly `chords`. Refuses, changing nothing, when one of them is already
    /// bound to another command; use [`ShortcutRegistry::rebind_taking`] to move it instead.
    /// An empty list unbinds the command.
    pub fn rebind(&mut self, id: &CommandId, chords: &[Chord]) -> Result<(), RegistryError> {
        if self.info(id).is_none() {
            return Err(RegistryError::Unknown(id.clone()));
        }
        for chord in chords {
            if let Some(by) = self.command_for(*chord).filter(|by| *by != id) {
                return Err(RegistryError::Taken { chord: *chord, by: by.clone() });
            }
        }
        self.set_override(id, chords);
        Ok(())
    }

    /// Binds `id` to exactly `chords`, removing each of them from any other command that had it.
    /// Returns the commands that lost a chord, so the UI can say so.
    pub fn rebind_taking(&mut self, id: &CommandId, chords: &[Chord]) -> Result<Vec<CommandId>, RegistryError> {
        if self.info(id).is_none() {
            return Err(RegistryError::Unknown(id.clone()));
        }
        let platform = self.platform;
        let wanted: Vec<ResolvedChord> = chords.iter().map(|c| c.resolve(platform)).collect();
        let others: Vec<CommandId> = self.commands.iter().map(|c| c.id.clone()).filter(|other| other != id).collect();
        let mut losers = Vec::new();
        for other in others {
            let current = self.bindings(&other);
            if current.iter().any(|c| wanted.contains(&c.resolve(platform))) {
                let kept: Vec<Chord> = current.iter().copied().filter(|c| !wanted.contains(&c.resolve(platform))).collect();
                self.set_override(&other, &kept);
                losers.push(other);
            }
        }
        self.set_override(id, chords);
        Ok(losers)
    }

    /// Restores `id`'s default chords.
    pub fn reset(&mut self, id: &CommandId) {
        self.overrides.remove(id);
    }

    /// Restores every command's defaults.
    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    /// Every pair of commands bound to the same keys on this platform, including a command
    /// bound twice to keys that are the same here (reported with itself as both sides).
    pub fn conflicts(&self) -> Vec<Conflict> {
        let mut seen: BTreeMap<ResolvedChord, (&CommandId, Chord)> = BTreeMap::new();
        let mut found = Vec::new();
        for info in &self.commands {
            for chord in self.bindings(&info.id) {
                let resolved = chord.resolve(self.platform);
                match seen.get(&resolved) {
                    Some((first, first_chord)) => {
                        found.push(Conflict { chord: *first_chord, first: (*first).clone(), second: info.id.clone() });
                    }
                    None => {
                        seen.insert(resolved, (&info.id, *chord));
                    }
                }
            }
        }
        found
    }

    /// Consumes the key presses of this frame that match a binding and returns the commands they
    /// trigger, in the order checked (most modifiers first). Call it once per frame, after
    /// widgets that need the keys (such as a focused text field) have had their turn:
    ///
    /// ```no_run
    /// # let ctx = egui::Context::default();
    /// # let registry = craft_ui::shortcuts::ShortcutRegistry::default();
    /// for command in ctx.input_mut(|i| registry.consume(i)) {
    ///     // run(command)
    /// }
    /// ```
    pub fn consume(&self, input: &mut InputState) -> Vec<CommandId> {
        let mut candidates: Vec<(&CommandId, Chord)> =
            self.commands.iter().flat_map(|info| self.bindings(&info.id).iter().map(move |chord| (&info.id, *chord))).collect();
        candidates.sort_by_key(|(_, chord)| std::cmp::Reverse(chord.specificity(self.platform)));
        let mut fired = Vec::new();
        for (id, chord) in candidates {
            if input.consume_shortcut(&chord.to_egui(self.platform)) && !fired.contains(id) {
                fired.push(id.clone());
            }
        }
        fired
    }

    /// The user's rebindings in portable text form, for saving in settings. Commands at their
    /// defaults are left out, so changed defaults in a later version reach the user.
    pub fn overrides(&self) -> BTreeMap<String, Vec<String>> {
        self.overrides.iter().map(|(id, chords)| (id.as_str().to_owned(), chords.iter().map(Chord::portable).collect())).collect()
    }

    /// Applies saved rebindings. Unknown commands and unreadable chords are skipped and
    /// reported; everything else applies. Saved rebindings are trusted as a whole, so they may
    /// introduce conflicts; check [`ShortcutRegistry::conflicts`] afterwards.
    pub fn apply_overrides(&mut self, saved: &BTreeMap<String, Vec<String>>) -> Vec<OverrideProblem> {
        let mut problems = Vec::new();
        for (id_text, chord_texts) in saved {
            let Some(id) = self.find(id_text).map(|c| c.id.clone()) else {
                problems.push(OverrideProblem::UnknownCommand(id_text.clone()));
                continue;
            };
            let mut chords = Vec::new();
            for text in chord_texts {
                match Chord::parse(text) {
                    Ok(chord) => chords.push(chord),
                    Err(error) => problems.push(OverrideProblem::BadChord { command: id.clone(), text: text.clone(), error }),
                }
            }
            self.set_override(&id, &chords);
        }
        problems
    }

    fn set_override(&mut self, id: &CommandId, chords: &[Chord]) {
        let mut chords = chords.to_vec();
        dedupe(&mut chords, self.platform);
        let same_as_default = self.info(id).is_some_and(|c| c.defaults == chords);
        if same_as_default {
            self.overrides.remove(id);
        } else {
            self.overrides.insert(id.clone(), chords);
        }
    }
}

/// Removes chords that press the same keys as an earlier one on `platform`, keeping order.
fn dedupe(chords: &mut Vec<Chord>, platform: Platform) {
    let mut seen = Vec::new();
    chords.retain(|c| {
        let r = c.resolve(platform);
        if seen.contains(&r) {
            false
        } else {
            seen.push(r);
            true
        }
    });
}

fn title_case(part: &str) -> String {
    let mut chars = part.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
        None => String::new(),
    }
}
