//! Frame texture management for the [`Renderer`](super::Renderer): lazily
//! (re)creating the GPU texture on size changes and uploading pixel data.

use crate::decoder::DecodedFrame;

/// A texture sized to the current frame, with its bind group.
pub(super) struct CachedTexture {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) texture: wgpu::Texture,
    pub(super) bind_group: wgpu::BindGroup,
}

impl super::Renderer {
    /// (Re)creates the frame texture when the size changes, then uploads pixels.
    pub(super) fn upload(&mut self, frame: &DecodedFrame) {
        let needs_new = match &self.texture {
            Some(cached) => cached.width != frame.width || cached.height != frame.height,
            None => true,
        };
        if needs_new {
            self.texture = Some(self.create_texture(frame.width, frame.height));
        }

        if let Some(cached) = &self.texture {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &cached.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &frame.pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.width * 4),
                    rows_per_image: Some(frame.height),
                },
                wgpu::Extent3d {
                    width: frame.width,
                    height: frame.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    fn create_texture(&self, width: u32, height: u32) -> CachedTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame-bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        CachedTexture {
            width,
            height,
            texture,
            bind_group,
        }
    }
}
