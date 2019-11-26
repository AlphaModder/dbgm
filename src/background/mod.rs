use image::{self, ImageResult, DynamicImage, GenericImageView};
use bitflags::bitflags;

use crate::math::Vec2;
use crate::sources::{OriginalKey, CompareKey, KeyRelation};

mod set;
pub use set::BackgroundSet;

struct EditInfo { pub center: Vec2, pub scale: f32 }

pub enum OriginalMeta {
    Known { size: (u32, u32) },
    Unavailable { last_known_size: Option<(u32, u32)> },
}

impl OriginalMeta {
    fn load(original: &dyn Original, old: Option<&OriginalMeta>) -> OriginalMeta {
        match original.read_image() {
            Ok(image) => OriginalMeta::Known { size: image.dimensions() },
            _ => OriginalMeta::Unavailable { last_known_size: old.and_then(|meta| meta.last_known_size()) },
        }
    }

    pub fn last_known_size(&self) -> Option<(u32, u32)> {
        match self {
            OriginalMeta::Known { size } => Some(*size),
            OriginalMeta::Unavailable { last_known_size } => *last_known_size,
        }
    }
}

bitflags! {
    pub struct DesktopBackgroundFlags: u32 {
        /// This background has not been edited since its original last changed.
        const UNEDITED = 0x1;
        /// This background's original has been deleted or altered.
        const ORIGINAL_MISSING = 0x2;
        // /// This background's original is temporarily or permanently unavailable.
        // const ORIGINAL_UNAVAILABLE = 0x4;
        /// This background has been excluded from the set and will be hidden by default.
        const EXCLUDED = 0x8;
    }
}

pub struct DesktopBackground {
    pub name: String,
    pub location: String,
    pub comments: Vec<String>,
    pub source: usize,
    pub original: OriginalKey,
    pub flags: DesktopBackgroundFlags,
    pub original_meta: OriginalMeta, // TODO: Should this use an immutable accessor or be public?
    edit_info: Option<EditInfo>,
}

impl DesktopBackground {
    /// Create a new DesktopBackground from an Original.
    pub fn from_original(source: usize, key: OriginalKey, original: &dyn Original) -> DesktopBackground {
        DesktopBackground {
            name: original.name(),
            location: original.location(), // TODO: Figure out how this should work
            comments: Vec::new(),
            source: source,
            original: key,
            flags: DesktopBackgroundFlags::UNEDITED,
            original_meta: OriginalMeta::load(original, None),
            edit_info: None,
        }
    }

    /// Update this background when changes have been made to its original. 
    pub fn update_from(&mut self, key: OriginalKey, original: &dyn Original) {
        assert!(key.compare(&self.original) != KeyRelation::Distinct);
        self.name = original.name();
        self.location = original.location();
        self.original = key;
        let last_size = self.original_meta.last_known_size();
        self.original_meta = OriginalMeta::load(original, Some(&self.original_meta));
        if self.original_meta.last_known_size() != last_size { // TODO: Might be better conditions here
            self.edit_info = None;
            self.flags.insert(DesktopBackgroundFlags::UNEDITED);
        }
    }

    /// Returns true if the original image file for this background cannot be accessed.
    pub fn is_unavailable(&self) -> bool {
        match self.original_meta {
            OriginalMeta::Unavailable { .. } => true,
            OriginalMeta::Known { .. } => false,
        }
    }

    /// Marks the background as unavailable, implying that its original image file cannot be accessed.
    pub fn mark_unavailable(&mut self) {
        self.original_meta = OriginalMeta::Unavailable {
            last_known_size: self.original_meta.last_known_size()
        };
    }

    /// Helper function to try reading this background's original. It is a logic error to call this with
    /// a different original than the one actually associated with the background.
    pub fn try_read_image_from(&mut self, original: &dyn Original) -> ImageResult<DynamicImage> {
        let image = original.read_image();
        // TODO: Should we *always* reload here, or just when there's an error? ConfirmChanges?
        if image.is_err() { self.original_meta = OriginalMeta::load(original, Some(&self.original_meta)); }
        image
    }

    /// The return value allows the crop region of this background to be edited, so as long as its original is
    /// not unavailable. See `is_unavailable` above.
    pub fn edit_crop_region(&mut self, crop_size: Vec2) -> Result<CropRegion, ()> {
        match self.original_meta {
            OriginalMeta::Known { size } => {
                let size = vec2![size.0 as f32, size.1 as f32];
                let edit_info = self.edit_info.get_or_insert_with(|| EditInfo { center: size / 2.0 + [0.5, 0.5], scale: 1.0 });
                let mut region = CropRegion {
                    crop_size: crop_size,
                    tex_size: size,
                    center: &mut edit_info.center,
                    scale: &mut edit_info.scale,
                };
                region.clip();
                Ok(region)
            },
            _ => Err(()) // TODO: Add error details
        }
    }
}

pub struct CropRegion<'a> {
    crop_size: Vec2, // The base size of the crop region (will be multiplied by scale)
    tex_size: Vec2, // The size of the texture being cropped
    pub center: &'a mut Vec2,
    pub scale: &'a mut f32,
}

impl<'a> CropRegion<'a> {
    pub fn top_left(&self) -> Vec2 {
        *self.center - (*self.scale * self.crop_size / 2.0)
    }

    pub fn bottom_right(&self) -> Vec2 {
        *self.center + (*self.scale * self.crop_size / 2.0)
    }

    pub fn clip(&mut self) {
        let size_ratio = self.tex_size.scale_inv(self.crop_size);
        *self.scale = f32::min(*self.scale, f32::min(size_ratio.x, size_ratio.y));
        let quarter = *self.scale * self.crop_size / 2.0;
        let center_min = vec2![0.0, 0.0] + quarter;
        let center_max = self.tex_size - quarter;
        *self.center = Vec2::min(center_max, Vec2::max(center_min, *self.center));
    }
}

pub trait Original {
    fn read_image(&self) -> ImageResult<DynamicImage>;
    fn name(&self) -> String;
    fn location(&self) -> String;
}