use std::ops::{Deref, DerefMut};

pub struct Settings<'h, 's> {
    settings: lol_html::Settings<'h, 's>,
}

impl<'h, 's> Default for Settings<'h, 's> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'h, 's> Settings<'h, 's> {
    pub fn new() -> Self {
        Self { settings: lol_html::Settings::default() }
    }

    pub fn into_inner(self) -> lol_html::Settings<'h, 's> {
        self.settings
    }
}

impl<'h, 's> Deref for Settings<'h, 's> {
    type Target = lol_html::Settings<'h, 's>;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

impl<'h, 's> DerefMut for Settings<'h, 's> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.settings
    }
}