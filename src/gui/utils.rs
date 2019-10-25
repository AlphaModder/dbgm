use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;

use imgui::*;

use super::modals::ErrorModal;

pub const AUTO_SIZE: [f32; 2] = [0.0, 0.0];

#[derive(Copy, Clone)]
pub struct TextureInfo {
    pub size: [f32; 2],
}

pub trait Textures {
    type CreationError: std::fmt::Debug;
    fn create_texture(&mut self, image: &image::DynamicImage) -> Result<TextureId, Self::CreationError>;
    fn texture_info(&self, texture: TextureId) -> Option<TextureInfo>;
}

pub struct ImageCache<K: Hash + Eq> {
    images: HashMap<K, (image::DynamicImage, Option<TextureId>)>,
}

impl<K: Hash + Eq> ImageCache<K> {
    pub fn new() -> Self { 
        ImageCache { images: HashMap::new() }
    }

    pub fn contains_image(&self, key: &K) -> bool {
        self.images.contains_key(key)
    }

    pub fn insert_image(&mut self, key: K, image: image::DynamicImage) {
        self.images.insert(key, (image, None));
    }

    pub fn remove_image(&mut self, key: &K) -> Option<(image::DynamicImage, Option<TextureId>)> {
        self.images.remove(key)
    }

    pub fn get_image(&self, key: &K) -> Option<&image::DynamicImage> {
        self.images.get(key).map(|(i, _)| i)
    }

    pub fn load_texture<T: Textures + ?Sized>(&mut self, key: &K, textures: &mut T) -> Option<Result<TextureId, T::CreationError>> {
        match self.images.get_mut(key) {
            Some((_, Some(texture))) => Some(Ok(*texture)),
            Some((image, texture_slot)) => {
                let texture = match textures.create_texture(image) {
                    Ok(texture) => texture,
                    Err(e) => return Some(Err(e))
                };
                *texture_slot = Some(texture);
                self.load_texture(key, textures)
            },
            None => None
        }
    }
}

pub trait UiExt {
    fn pad_to_center_h(&self, width: f32);
    fn pad_to_center_v(&self, height: f32);
    fn is_popup_open(&self, popup: &ImStr) -> bool;
    fn button_hack(&self, label: &ImStr, size: [f32; 2], enabled: bool) -> bool;
    fn toggle_button(&self, id: &ImStr, text_on: &str, text_off: &str, value: &mut bool);
    fn move_cursor(&self, amount: [f32; 2]);
}

impl<'ui> UiExt for Ui<'ui> {
    fn pad_to_center_h(&self, width: f32) {
        self.move_cursor([(self.content_region_avail()[0] - width) / 2.0, 0.0]);
    }

    fn pad_to_center_v(&self, height: f32) {
        self.move_cursor([0.0, (self.content_region_avail()[1] - height) / 2.0])
    }

    fn is_popup_open(&self, popup: &ImStr) -> bool {
        unsafe { imgui::sys::igIsPopupOpen(popup.as_ptr()) }
    }

    // TODO: Replace this when ImGui supports proper disabled widgets.
    fn button_hack(&self, label: &ImStr, size: [f32; 2], enabled: bool) -> bool {
        match enabled {
            true => self.button(label, size),
            false => {
                let style = self.push_style_var(StyleVar::Alpha(self.clone_style().alpha * 0.5));
                let colors = self.push_style_colors(&[
                    (StyleColor::ButtonActive, self.style_color(StyleColor::Button)),
                    (StyleColor::ButtonHovered, self.style_color(StyleColor::Button)),
                ]);
                self.button(label, size);
                style.pop(self);
                colors.pop(self);
                false
            }
        }
    }

    fn toggle_button(&self, id: &ImStr, text_on: &str, text_off: &str, value: &mut bool) {
        let label = if *value { text_on } else { text_off };
        if self.button(&im_str!("{}###{}", label, id), AUTO_SIZE) {
            *value = !*value;
        }
    }

    fn move_cursor(&self, amount: [f32; 2]) {
        let cursor_pos = self.cursor_pos();
        self.set_cursor_pos([cursor_pos[0] + amount[0], cursor_pos[1] + amount[1]]);
    }
}

pub trait ChildWindowExt: Sized {
    fn border_box(self, ui: &Ui, size: [f32; 2]) -> Self;
}

impl<'a> ChildWindowExt for ChildWindow<'a> {
    fn border_box(self, ui: &Ui, size: [f32; 2]) -> Self {
        let border_size = ui.clone_style().child_border_size;
        self.size([f32::max(0.0, size[0] - border_size), f32::max(0.0, size[1] - border_size)])
    }
}

pub fn choose_folder(desc: &str) -> Result<Option<PathBuf>, ErrorModal> {
    match nfd::open_pick_folder(None) {
        Ok(nfd::Response::Okay(f)) => match f.parse() {
            Ok(path) => Ok(Some(path)),
            Err(e) => return Err(ErrorModal::new(format!("Invalid path to {}.", desc), Some(e))),
        }
        Err(e) => return Err(ErrorModal::new(format!("Could not open {} picker.", desc), Some(e))),
        _ => Ok(None),
    }
}

pub fn fit_size(original: [f32; 2], bounds: [f32; 2]) -> [f32; 2] {
    let scale_factor = f32::min(bounds[0] / original[0], bounds[1] / original[1]);
    [original[0] * scale_factor, original[1] * scale_factor]
}