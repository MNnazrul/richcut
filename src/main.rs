mod clipboard;
mod history;
mod storage;
mod system;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSPasteboard, NSRunningApplication};
use slint::{ComponentHandle, ModelRc, PlatformError, VecModel};

use history::History;
use storage::Storage;

slint::include_modules!();

const PICKER_WIDTH: f64 = 420.0;
const PICKER_HEIGHT: f64 = 520.0;
const PASTE_DELAY: Duration = Duration::from_millis(150);

struct Picker {
    app: slint::Weak<MainWindow>,
    mtm: MainThreadMarker,
    history: RefCell<History>,
    pasteboard: Retained<NSPasteboard>,
    last_change: Cell<isize>,
    previous_app: RefCell<Option<Retained<NSRunningApplication>>>,
    became_active: Cell<bool>,
    storage: Option<Storage>,
    saved_position: Cell<Option<slint::LogicalPosition>>,
    dragged: Cell<bool>,
}

impl Picker {
    fn toggle(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };

        if app.window().is_visible() {
            self.hide(true);
        } else {
            self.show(&app);
        }
    }

    fn show(&self, app: &MainWindow) {
        *self.previous_app.borrow_mut() = system::frontmost_other_app();

        app.set_search_text("".into());
        {
            let mut history = self.history.borrow_mut();
            history.set_query("");
            history.refresh(app, None);
        }

        let position = self
            .saved_position
            .get()
            .filter(|position| system::is_on_screen(self.mtm, *position))
            .or_else(|| system::picker_position(self.mtm, PICKER_WIDTH, PICKER_HEIGHT));

        if let Some(position) = position {
            app.window().set_position(position);
        }

        if let Err(error) = app.show() {
            eprintln!("richcut: failed to show window: {error}");
            return;
        }

        system::configure_window(app.window());
        system::activate_self(self.mtm);
        self.became_active.set(false);
        app.invoke_focus_search();
        app.invoke_ensure_selected_visible();
    }

    fn hide(&self, restore_focus: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };

        let _ = app.hide();
        self.became_active.set(false);

        let previous = self.previous_app.borrow_mut().take();

        if restore_focus && let Some(previous) = previous {
            system::activate_app(&previous);
        }
    }

    fn drag(&self, dx: f32, dy: f32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };

        let window = app.window();
        let scale = window.scale_factor();
        let position = window.position();

        window.set_position(slint::PhysicalPosition::new(
            position.x + (dx * scale).round() as i32,
            position.y + (dy * scale).round() as i32,
        ));
        self.dragged.set(true);
    }

    fn drag_finished(&self) {
        if !self.dragged.replace(false) {
            return;
        }

        let Some(app) = self.app.upgrade() else {
            return;
        };

        let window = app.window();
        let position = window.position().to_logical(window.scale_factor());
        self.saved_position.set(Some(position));

        if let Some(storage) = &self.storage
            && let Err(error) = storage.save_window_position(position.x, position.y)
        {
            eprintln!("richcut: failed to save window position: {error}");
        }
    }

    fn search(&self, query: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };

        let mut history = self.history.borrow_mut();
        history.set_query(query);
        history.refresh(&app, None);
    }

    fn paste(&self, source: usize) {
        let Some(app) = self.app.upgrade() else {
            return;
        };

        let Some(content) = self.history.borrow().content(source).cloned() else {
            return;
        };

        clipboard::write(&self.pasteboard, &content);
        self.last_change.set(self.pasteboard.changeCount());
        self.history.borrow_mut().move_to_front(source);

        let target = self.previous_app.borrow_mut().take();

        let _ = app.hide();
        self.became_active.set(false);

        {
            let mut history = self.history.borrow_mut();
            history.set_query("");
            history.refresh(&app, None);
        }

        if let Some(target) = target {
            system::activate_app(&target);

            if system::is_accessibility_trusted() {
                slint::Timer::single_shot(PASTE_DELAY, system::send_paste_keystroke);
            }
        }
    }

    fn poll(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };

        if app.window().is_visible() {
            if system::is_self_active(self.mtm) {
                self.became_active.set(true);
            } else if self.became_active.get() {
                self.hide(false);
            }
        }

        let current_count = self.pasteboard.changeCount();
        if current_count == self.last_change.get() {
            return;
        }

        self.last_change.set(current_count);

        if clipboard::is_private(&self.pasteboard) {
            return;
        }

        let content = clipboard::fetch(&self.pasteboard);
        let mut history = self.history.borrow_mut();
        let keep = history
            .source_at_row(app.get_selected_index())
            .map(|source| source + 1);

        if history.push_front(content) {
            history.refresh(&app, keep);
        }
    }
}

fn main() -> Result<(), PlatformError> {
    let mtm = MainThreadMarker::new().expect("richcut must run on the main thread");
    let app = MainWindow::new()?;

    let rows = Rc::new(VecModel::<ClipRow>::default());
    app.set_clipboard_items(ModelRc::from(rows.clone()));

    let storage = Storage::new();
    let history = History::new(rows, storage.clone());
    history.refresh(&app, None);

    let saved_position = storage
        .as_ref()
        .and_then(Storage::load_window_position)
        .map(|(x, y)| slint::LogicalPosition::new(x, y));

    let pasteboard = NSPasteboard::generalPasteboard();

    let picker = Rc::new(Picker {
        app: app.as_weak(),
        mtm,
        history: RefCell::new(history),
        last_change: Cell::new(pasteboard.changeCount()),
        pasteboard,
        previous_app: RefCell::new(None),
        became_active: Cell::new(false),
        storage,
        saved_position: Cell::new(saved_position),
        dragged: Cell::new(false),
    });

    app.on_window_dragged({
        let picker = picker.clone();
        move |dx, dy| picker.drag(dx, dy)
    });

    app.on_drag_finished({
        let picker = picker.clone();
        move || picker.drag_finished()
    });

    app.on_toggle_picker({
        let picker = picker.clone();
        move || picker.toggle()
    });

    app.on_close_requested({
        let picker = picker.clone();
        move || picker.hide(true)
    });

    app.on_search_changed({
        let picker = picker.clone();
        move |query| picker.search(&query)
    });

    app.on_paste_item({
        let picker = picker.clone();
        move |source| {
            if let Ok(source) = usize::try_from(source) {
                picker.paste(source);
            }
        }
    });

    app.on_quit_requested(|| {
        let _ = slint::quit_event_loop();
    });

    let hotkey_manager =
        GlobalHotKeyManager::new().map_err(|error| PlatformError::Other(error.to_string()))?;
    let hotkey = HotKey::new(Some(Modifiers::ALT), Code::KeyV);
    hotkey_manager
        .register(hotkey)
        .map_err(|error| PlatformError::Other(error.to_string()))?;

    let app_weak = app.as_weak();
    let hotkey_id = hotkey.id();

    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.id == hotkey_id && event.state == HotKeyState::Pressed {
            let _ = app_weak.upgrade_in_event_loop(|app| app.invoke_toggle_picker());
        }
    }));

    if !system::request_accessibility() {
        eprintln!(
            "richcut: grant Accessibility access (System Settings > Privacy & Security > Accessibility) so Enter can paste into the previous app"
        );
    }

    slint::Timer::single_shot(Duration::ZERO, move || system::set_accessory_policy(mtm));

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(150), {
        let picker = picker.clone();
        move || picker.poll()
    });

    eprintln!("richcut is running. Press Option+V to open clipboard history.");

    slint::run_event_loop_until_quit()
}
