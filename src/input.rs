use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, HtmlCanvasElement, KeyboardEvent, MouseEvent, WheelEvent, Window};

use crate::AppState;

/// Helper to get window and document, returning None if unavailable
fn get_window_document() -> Option<(Window, Document)> {
    let window = web_sys::window()?;
    let document = window.document()?;
    Some((window, document))
}

/// Tracks keyboard and mouse input state
pub struct InputState {
    pub keys: HashSet<String>,
    pub mouse_delta_x: f32,
    pub mouse_delta_y: f32,
    pub mouse_locked: bool,
    pub scroll_delta: f32,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
            mouse_locked: false,
            scroll_delta: 0.0,
        }
    }

    pub fn is_key_down(&self, key: &str) -> bool {
        self.keys.contains(key)
    }

    pub fn clear_frame_state(&mut self) {
        self.mouse_delta_x = 0.0;
        self.mouse_delta_y = 0.0;
        self.scroll_delta = 0.0;
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

/// Setup all input event handlers
pub fn setup_input_handlers(canvas: &HtmlCanvasElement, state: Rc<RefCell<AppState>>) {
    let Some((_window, document)) = get_window_document() else {
        log::warn!("Could not access window/document for input handlers");
        return;
    };

    // Keyboard down
    {
        let state = Rc::clone(&state);
        let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let key = normalize_key(&event.key());
            // Prevent default for game keys
            if matches!(
                key.as_str(),
                "w" | "a" | "s" | "d" | " " | "shift" | "tab" | "r" | "q" | "e" | "u" | "i" | "o" | "j" | "k" | "l"
            ) {
                event.prevent_default();
            }
            let mut state = state.borrow_mut();
            state.input_mut().keys.insert(key);
        }) as Box<dyn FnMut(_)>);

        let _ = document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // Keyboard up
    {
        let state = Rc::clone(&state);
        let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
            let key = normalize_key(&event.key());
            state.borrow_mut().input_mut().keys.remove(&key);
        }) as Box<dyn FnMut(_)>);

        let _ = document.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // Mouse move
    {
        let state = Rc::clone(&state);
        let closure = Closure::wrap(Box::new(move |event: MouseEvent| {
            let mut state = state.borrow_mut();
            let input = state.input_mut();
            if input.mouse_locked {
                input.mouse_delta_x += event.movement_x() as f32;
                input.mouse_delta_y += event.movement_y() as f32;
            }
        }) as Box<dyn FnMut(_)>);

        let _ = document.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // Mouse wheel (zoom)
    {
        let state = Rc::clone(&state);
        let closure = Closure::wrap(Box::new(move |event: WheelEvent| {
            event.prevent_default();
            let mut state = state.borrow_mut();
            // Normalize scroll: positive delta_y = scroll down = zoom out
            state.input_mut().scroll_delta += event.delta_y() as f32 * 0.01;
        }) as Box<dyn FnMut(_)>);

        let _ = canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // Click to lock pointer
    {
        let canvas_clone = canvas.clone();
        let closure = Closure::wrap(Box::new(move |_: MouseEvent| {
            let _ = canvas_clone.request_pointer_lock();
        }) as Box<dyn FnMut(_)>);

        let _ = canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref());
        closure.forget();
    }

    // Pointer lock change
    {
        let state = Rc::clone(&state);
        let canvas_clone = canvas.clone();
        let closure = Closure::wrap(Box::new(move || {
            let Some((_window, document)) = get_window_document() else { return };
            let locked = document
                .pointer_lock_element()
                .and_then(|el| {
                    canvas_clone
                        .dyn_ref::<web_sys::Element>()
                        .map(|canvas_el| el == *canvas_el)
                })
                .unwrap_or(false);
            state.borrow_mut().input_mut().mouse_locked = locked;
            if locked {
                log::info!("Pointer locked - use WASD to move, mouse to look");
            }
        }) as Box<dyn FnMut()>);

        let _ = document.add_event_listener_with_callback("pointerlockchange", closure.as_ref().unchecked_ref());
        closure.forget();
    }
}

/// Normalize key names for consistent handling
fn normalize_key(key: &str) -> String {
    match key {
        "Shift" => "shift".to_string(),
        "Control" => "control".to_string(),
        "Alt" => "alt".to_string(),
        " " => " ".to_string(), // Keep space as is
        k => k.to_lowercase(),
    }
}
