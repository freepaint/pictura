use crate::image::Channel;
use nalgebra::Vector4;

pub trait Shader: Fn(&Layer, Vector4<f32>) -> Vector4<f32> + Send + Sync {}

impl<T> Shader for T where T: Fn(&Layer, Vector4<f32>) -> Vector4<f32> + Send + Sync {}

pub struct Layer {
    // TODO: Investigate better IDs then String
    channels: fxhash::FxHashMap<String, Channel>,
    width: usize,
    height: usize,
}

impl Layer {
    pub fn new(width: usize, height: usize, layer: &[&str]) -> Self {
        Self {
            channels: layer
                .iter()
                .map(|name| (name.to_string(), Channel::new(width, height)))
                .collect(),
            width,
            height,
        }
    }

    pub fn new_gray(width: usize, height: usize) -> Self {
        Self::new(width, height, &["gray"])
    }

    pub fn new_rgb(width: usize, height: usize) -> Self {
        Self::new(width, height, &["red", "green", "blue"])
    }

    pub fn new_rgba(width: usize, height: usize) -> Self {
        Self::new(width, height, &["red", "green", "blue", "alpha"])
    }

    pub fn channel(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }

    pub fn get_channel<S: ToString>(&self, key: S) -> Option<&Channel> {
        self.channels.get(&key.to_string())
    }

    pub fn get_channel_mut<S: ToString>(&mut self, key: S) -> Option<&mut Channel> {
        self.channels.get_mut(&key.to_string())
    }

    pub fn templated_update<F>(shader: impl Shader, _template: &Layer) -> Layer {
        let _shader = Box::new(shader) as Box<dyn Shader>;

        todo!()
    }
}
