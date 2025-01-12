//! Uploads data to the GPU which is needed for rendering.

use crate::context::MapContext;
use crate::coords::{ViewRegion, Zoom};
use crate::io::tile_cache::TileCache;
use crate::io::LayerTessellateMessage;
use crate::render::camera::ViewProjection;
use crate::render::resource::IndexEntry;
use crate::render::shaders::{
    ShaderCamera, ShaderFeatureStyle, ShaderGlobals, ShaderLayerMetadata, Vec4f32,
};
use crate::render::tile_view_pattern::TileInView;
use crate::render::util::Eventually::Initialized;
use crate::schedule::Stage;
use crate::{RenderState, Renderer, Style};

use std::iter;

#[derive(Default)]
pub struct UploadStage;

impl Stage for UploadStage {
    #[tracing::instrument(name = "UploadStage", skip_all)]
    fn run(
        &mut self,
        MapContext {
            view_state,
            style,
            tile_cache,
            renderer:
                Renderer {
                    settings: _,
                    device: _,
                    queue,
                    surface: _,
                    state,
                    ..
                },
            ..
        }: &mut MapContext,
    ) {
        let visible_level = view_state.visible_level();

        let view_proj = view_state.view_projection();

        if let Initialized(globals_bind_group) = &state.globals_bind_group {
            // Update globals
            queue.write_buffer(
                &globals_bind_group.uniform_buffer,
                0,
                bytemuck::cast_slice(&[ShaderGlobals::new(ShaderCamera::new(
                    view_proj.downcast().into(),
                    view_state
                        .camera
                        .position
                        .to_homogeneous()
                        .cast::<f32>()
                        .unwrap()
                        .into(),
                ))]),
            );
        }

        let view_region = view_state
            .camera
            .view_region_bounding_box(&view_proj.invert())
            .map(|bounding_box| ViewRegion::new(bounding_box, 0, *view_state.zoom, visible_level));

        if let Some(view_region) = &view_region {
            let zoom = view_state.zoom();

            self.upload_tile_geometry(state, queue, tile_cache, style, view_region);
            self.update_tile_view_pattern(state, queue, view_region, &view_proj, zoom);
            self.update_metadata();
        }
    }
}

impl UploadStage {
    #[tracing::instrument(skip_all)]
    pub(crate) fn update_metadata(&self) {
        /*let animated_one = 0.5
        * (1.0
            + ((SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64()
                * 10.0)
                .sin()));*/

        // Factor which determines how much we need to adjust the width of lines for example.
        // If zoom == z -> zoom_factor == 1

        /*  for entries in self.buffer_pool.index().iter() {
        for entry in entries {
            let world_coords = entry.coords;*/

        // TODO: Update features
        /*let source_layer = entry.style_layer.source_layer.as_ref().unwrap();

        if let Some(result) = scheduler
            .get_tile_cache()
            .iter_tessellated_layers_at(&world_coords)
            .unwrap()
            .find(|layer| source_layer.as_str() == layer.layer_name())
        {
            let color: Option<Vec4f32> = entry
                .style_layer
                .paint
                .as_ref()
                .and_then(|paint| paint.get_color())
                .map(|mut color| {
                    color.color.b = animated_one as f32;
                    color.into()
                });

            match result {
                LayerTessellateResult::UnavailableLayer { .. } => {}
                LayerTessellateResult::TessellatedLayer {
                    layer_data,
                    feature_indices,
                    ..
                } => {

                    let feature_metadata = layer_data
                        .features()
                        .iter()
                        .enumerate()
                        .flat_map(|(i, _feature)| {
                            iter::repeat(ShaderFeatureStyle {
                                color: color.unwrap(),
                            })
                            .take(feature_indices[i] as usize)
                        })
                        .collect::<Vec<_>>();

                    self.buffer_pool.update_feature_metadata(
                        &self.queue,
                        entry,
                        &feature_metadata,
                    );
                }
            }
        }*/
        /*            }
        }*/
    }

    #[tracing::instrument(skip_all)]
    pub fn update_tile_view_pattern(
        &self,
        RenderState {
            tile_view_pattern,
            buffer_pool,
            ..
        }: &mut RenderState,
        queue: &wgpu::Queue,
        view_region: &ViewRegion,
        view_proj: &ViewProjection,
        zoom: Zoom,
    ) {
        if let (Initialized(tile_view_pattern), Initialized(buffer_pool)) =
            (tile_view_pattern, buffer_pool)
        {
            tile_view_pattern.update_pattern(view_region, buffer_pool, zoom);
            tile_view_pattern.upload_pattern(queue, view_proj);
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn upload_tile_geometry(
        &self,
        RenderState { buffer_pool, .. }: &mut RenderState,
        queue: &wgpu::Queue,
        tile_cache: &TileCache,
        style: &Style,
        view_region: &ViewRegion,
    ) {
        if let Initialized(buffer_pool) = buffer_pool {
            // Upload all tessellated layers which are in view
            for world_coords in view_region.iter() {
                let loaded_layers = buffer_pool
                    .get_loaded_layers_at(&world_coords)
                    .unwrap_or_default();
                if let Some(available_layers) = tile_cache
                    .iter_tessellated_layers_at(&world_coords)
                    .map(|layers| {
                        layers
                            .filter(|result| !loaded_layers.contains(&result.layer_name()))
                            .collect::<Vec<_>>()
                    })
                {
                    for style_layer in &style.layers {
                        let source_layer = style_layer.source_layer.as_ref().unwrap();

                        if let Some(message) = available_layers
                            .iter()
                            .find(|layer| source_layer.as_str() == layer.layer_name())
                        {
                            let color: Option<Vec4f32> = style_layer
                                .paint
                                .as_ref()
                                .and_then(|paint| paint.get_color())
                                .map(|color| color.into());

                            match message {
                                LayerTessellateMessage::UnavailableLayer { coords: _, .. } => {
                                    /*self.buffer_pool.mark_layer_unavailable(*coords);*/
                                }
                                LayerTessellateMessage::TessellatedLayer {
                                    coords,
                                    feature_indices,
                                    layer_data,
                                    buffer,
                                    ..
                                } => {
                                    let allocate_feature_metadata = tracing::span!(
                                        tracing::Level::TRACE,
                                        "allocate_feature_metadata"
                                    );

                                    let guard = allocate_feature_metadata.enter();
                                    let feature_metadata = layer_data
                                        .features
                                        .iter()
                                        .enumerate()
                                        .flat_map(|(i, _feature)| {
                                            iter::repeat(ShaderFeatureStyle {
                                                color: color.unwrap(),
                                            })
                                            .take(feature_indices[i] as usize)
                                        })
                                        .collect::<Vec<_>>();
                                    drop(guard);

                                    tracing::trace!("Allocating geometry at {}", &coords);
                                    buffer_pool.allocate_layer_geometry(
                                        queue,
                                        *coords,
                                        style_layer.clone(),
                                        buffer,
                                        ShaderLayerMetadata::new(style_layer.index as f32),
                                        &feature_metadata,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
