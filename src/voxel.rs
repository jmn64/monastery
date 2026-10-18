use std::collections::HashMap;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use bevy::tasks::ComputeTaskPool;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::image::{ImageSampler, ImageSamplerDescriptor, ImageFilterMode};
use crate::camera::{CurrentLevel, SliceSettings};
use crate::state::AppState;
use crate::terrain_gen::{apply_hydraulic_erosion, generate_base_noise, ErosionSettings, Heightfield2d, RockCategory};

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
pub const GRID_RADIUS: i32 = 8;
pub const MAP_CHUNKS: usize = (GRID_RADIUS * 2) as usize;
pub const MAP_DIMENSION: usize = MAP_CHUNKS * CHUNK_SIZE;

#[derive(Resource, Default)]
pub struct WorldChunkStorage {
    pub chunks: HashMap<(i32, i32), ChunkData>,
}

impl WorldChunkStorage {
    //noinspection ALL
    pub fn get(&self, cx: i32, cz: i32) -> Option<&ChunkData> {
        self.chunks.get(&(cx, cz))
    }
}

pub fn spawn_voxel_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut chunk_storage: ResMut<WorldChunkStorage>,
    erosion_settings: Res<ErosionSettings>,
    level: Res<CurrentLevel>,
    slice_settings: Res<SliceSettings>,
) {
    chunk_storage.chunks.clear();

    let atlas_handle = generate_voxel_atlas(&mut images);
    let material_handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(atlas_handle),
        perceptual_roughness: 0.85,
        ..default()
    });

    // allocate heightfield for world bounds
    let mut heightfield = Heightfield2d::new(MAP_DIMENSION);

    // Multi pass generation: noise base + hydraulic
    generate_base_noise(&mut heightfield, 6.0, (CHUNK_SIZE - 4) as f32);
    apply_hydraulic_erosion(&mut heightfield, &erosion_settings);

    let sea_level: usize = 10;
    let initial_max_y = if slice_settings.enabled {
        level.0 as usize
    } else {
        CHUNK_SIZE
    };

    // rasterize and mesh chunks from global heightfield
    for cx in -GRID_RADIUS..GRID_RADIUS {
        for cz in -GRID_RADIUS..GRID_RADIUS {
            let chunk_data = generate_terrain_chunk(cx, cz, &heightfield, sea_level);

            let mut min_y = CHUNK_SIZE;
            let mut max_y = 0;
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        if chunk_data.get(x, y, z).is_opaque() {
                            min_y = min_y.min(y);
                            max_y = max_y.max(y);
                        }
                    }
                }
            }
            let mesh = generate_sliced_chunk_mesh(&chunk_data, initial_max_y);

            let world_x = (cx * CHUNK_SIZE as i32) as f32;
            let world_z = (cz * CHUNK_SIZE as i32) as f32;

            commands.spawn((
                DespawnOnExit(AppState::InGame),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material_handle.clone()),
                Transform::from_xyz(world_x, 0.0, world_z),
                ChunkMeshMarker {
                    cx,
                    cz,
                    min_y,
                    max_y,
                    current_rendered_y: initial_max_y,
                },
                ));

            chunk_storage.chunks.insert((cx, cz), chunk_data);
        }
    }
}

pub fn update_level_mesh_culling(
    level: Res<CurrentLevel>,
    slice_settings: Res<SliceSettings>,
    chunk_storage: Res<WorldChunkStorage>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&mut ChunkMeshMarker, &Mesh3d)>,
) {
    if !level.is_changed() && !slice_settings.is_changed() {
        return;
    }

    let target_y = if slice_settings.enabled {
        (level.0.max(0) as usize).min(CHUNK_SIZE)
    } else {
        CHUNK_SIZE
    };

    // Collect chunks that actually visually changed
    let mut chunks_to_remesh = Vec::new();

    for (mut marker, mesh_handle) in &mut query {
        if marker.current_rendered_y == target_y {
            continue;
        }

        // if target_y above max_y and chunk already rendered to max_y, skip
        if target_y >= marker.max_y && marker.current_rendered_y >= marker.max_y {
            marker.current_rendered_y = target_y;
            continue;
        }

        // if target_y is below min_y and chunk is already empty, skip
        if target_y < marker.min_y && marker.current_rendered_y < marker.min_y {
            marker.current_rendered_y = target_y;
            continue;
        }

        marker.current_rendered_y = target_y;
        if let Some(chunk_data) = chunk_storage.get(marker.cx, marker.cz) {
            chunks_to_remesh.push((mesh_handle.clone(), chunk_data.clone()));
        }
    }

    if chunks_to_remesh.is_empty() {
        return;
    }

    // parallelize mesh buffer generation
    let pool = ComputeTaskPool::get();
    let generated_meshes: Vec<(Mesh3d, Mesh)> = pool.scope(|scope| {
        for (mesh_handle, chunk_data) in chunks_to_remesh {
            scope.spawn(async move {
                let mesh = generate_sliced_chunk_mesh(&chunk_data, target_y);
                (mesh_handle, mesh)
            });
        }
    });

    // mutate GPU asset handles on main thread
    for (handle, new_mesh) in generated_meshes {
        if let Some(mut mesh) = meshes.get_mut(&handle) {
            *mesh = new_mesh;
        }
    }
}

pub fn generate_terrain_chunk(
    chunk_x: i32,
    chunk_z: i32,
    field: &Heightfield2d,
    sea_level: usize,
) -> ChunkData {
    let mut chunk = ChunkData::default();

    for lx in 0..CHUNK_SIZE {
        for lz in 0..CHUNK_SIZE {
            // offset coordinates from [-GRID_RADIUS * CHUNK_SIZE] to [0..MAP_DIMENSION]
            let field_x = ((chunk_x + GRID_RADIUS) * CHUNK_SIZE as i32) + lx as i32;
            let field_z = ((chunk_z + GRID_RADIUS) * CHUNK_SIZE as i32) + lz as i32;

            let h_val = field.get_height(field_x as usize, field_z as usize);
            let ground_height = (h_val.round() as usize).clamp(1, CHUNK_SIZE - 2);

            let bedrock_cat = field.bedrock[field.index(field_x as usize, field_z as usize)];

            for ly in 0..CHUNK_SIZE {
                let voxel = if ly < ground_height.saturating_sub(3) {
                    match bedrock_cat {
                        RockCategory::Sedimentary => match ly % 8 {
                            0..=3 => VoxelType::Limestone,
                            4..=6 => VoxelType::Sandstone,
                            _ => VoxelType::Shale,
                        },
                        RockCategory::IgneousIntrusive => {
                            if (field_x + ly as i32 + field_z) % 5 == 0 { VoxelType::Diorite } else { VoxelType::Granite }
                        }
                        RockCategory::IgneousExtrusive => {
                            if ly < 4 { VoxelType::Obsidian } else { VoxelType::Basalt }
                        }
                        RockCategory::Metamorphic => {
                            if ly % 5 == 0 { VoxelType::Marble } else { VoxelType::Slate }
                        }
                    }
                } else if ly < ground_height {
                    VoxelType::Dirt
                } else if ly == ground_height {
                    if ly <= sea_level {
                        VoxelType::Dirt //shoreline/submerged
                    } else {
                        VoxelType::Grass
                    }
                } else if ly <= sea_level {
                    VoxelType::Water
                } else {
                    VoxelType::Air
                };

                chunk.set(lx, ly, lz, voxel);
            }
        }
    }

    chunk
}

pub const TILE_SIZE: usize = 16;
pub const ATLAS_TILES: usize = 15; // matches number of VoxelType variants

#[derive(Clone, Copy)]
pub enum PatternFamily {
    Solid,
    Soil { contrast: f32 },
    Stratified { band_freq: f32, band_contrast: f32, noise_contrast: f32 },
    Speckled { density: f32, contrast: f32 },
    FineGrain { contrast: f32, glossy_streak: bool },
    Foliated { vein_freq: f32, sharpness: f32, contrast: f32 },
    WoodGrain { fiber_freq: f32, contrast: f32 },
}

pub struct TextureRecipe {
    pub base_rgb: [f32; 3],
    pub secondary_rgb: [f32; 3], // for veins, speckles, strata bands
    pub family: PatternFamily,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VoxelType {
    Air = 0,
    Dirt = 1,
    Grass = 2,
    Water = 3,
    // Sedimentary Rock
    Limestone = 4,
    Sandstone = 5,
    Shale = 6,
    // Igneous Intrusive
    Granite = 7,
    Diorite = 8,
    // Igneous Extrusive
    Basalt = 9,
    Obsidian = 10,
    // Metamorphic
    Marble = 11,
    Slate = 12,
    // Wood
    Oak = 13,
    Pine = 14,
}

impl VoxelType {
    pub const ALL: [VoxelType; ATLAS_TILES] = [
        Self::Air, Self::Dirt, Self::Grass, Self::Water,
        Self::Limestone, Self::Sandstone, Self::Shale,
        Self::Granite, Self::Diorite,
        Self::Basalt, Self::Obsidian,
        Self::Marble, Self::Slate,
        Self::Oak, Self::Pine,
    ];
    #[inline(always)]
    pub fn is_opaque(&self) -> bool {
        *self != VoxelType::Air
    }

    pub fn atlas_index(&self) -> usize {
        *self as usize
    }

    pub fn recipe(&self) -> TextureRecipe {
        use PatternFamily::*;
        match self {
            VoxelType::Air => TextureRecipe {
                base_rgb: [0.0, 0.0, 0.0],
                secondary_rgb: [0.0, 0.0, 0.0],
                family: Solid,
            },
            VoxelType::Dirt => TextureRecipe {
                base_rgb: [0.42, 0.30, 0.20],
                secondary_rgb: [0.30, 0.21, 0.14],
                family: Soil { contrast: 0.35 },
            },
            VoxelType::Grass => TextureRecipe {
                base_rgb: [0.28, 0.52, 0.22],
                secondary_rgb: [0.20, 0.40, 0.15],
                family: Soil { contrast: 0.30 },
            },
            VoxelType::Water => TextureRecipe {
                base_rgb: [0.15, 0.38, 0.78],
                secondary_rgb: [0.22, 0.48, 0.85],
                family: Stratified { band_freq: 1.5, band_contrast: 0.2, noise_contrast: 0.05 },
            },
            // Sedimentary Rocks
            VoxelType::Limestone => TextureRecipe {
                base_rgb: [0.76, 0.74, 0.66],
                secondary_rgb: [0.64, 0.61, 0.53],
                family: Stratified { band_freq: 2.0, band_contrast: 0.4, noise_contrast: 0.15 },
            },
            VoxelType::Sandstone => TextureRecipe {
                base_rgb: [0.82, 0.68, 0.45],
                secondary_rgb: [0.70, 0.52, 0.32],
                family: Stratified { band_freq: 3.0, band_contrast: 0.5, noise_contrast: 0.20 },
            },
            VoxelType::Shale => TextureRecipe {
                base_rgb: [0.32, 0.34, 0.36],
                secondary_rgb: [0.22, 0.23, 0.25],
                family: Stratified { band_freq: 5.0, band_contrast: 0.6, noise_contrast: 0.10 },
            },
            // Igneous Intrusive Rocks
            VoxelType::Granite => TextureRecipe {
                base_rgb: [0.62, 0.48, 0.46],
                secondary_rgb: [0.25, 0.22, 0.22],
                family: Speckled { density: 0.35, contrast: 0.7 },
            },
            VoxelType::Diorite => TextureRecipe {
                base_rgb: [0.82, 0.82, 0.84],
                secondary_rgb: [0.18, 0.18, 0.20],
                family: Speckled { density: 0.45, contrast: 0.85 },
            },
            // Igneous Extrusive Rocks
            VoxelType::Basalt => TextureRecipe {
                base_rgb: [0.20, 0.21, 0.23],
                secondary_rgb: [0.12, 0.12, 0.14],
                family: FineGrain { contrast: 0.4, glossy_streak: false },
            },
            VoxelType::Obsidian => TextureRecipe {
                base_rgb: [0.08, 0.06, 0.12],
                secondary_rgb: [0.24, 0.18, 0.35],
                family: FineGrain { contrast: 0.2, glossy_streak: true },
            },
            // Metamorphic Rocks
            VoxelType::Marble => TextureRecipe {
                base_rgb: [0.92, 0.91, 0.89],
                secondary_rgb: [0.55, 0.56, 0.58],
                family: Foliated { vein_freq: 2.0, sharpness: 4.0, contrast: 0.7 },
            },
            VoxelType::Slate => TextureRecipe {
                base_rgb: [0.28, 0.31, 0.35],
                secondary_rgb: [0.19, 0.21, 0.24],
                family: Foliated { vein_freq: 4.0, sharpness: 1.5, contrast: 0.5 },
            },
            // Wood
            VoxelType::Oak => TextureRecipe {
                base_rgb: [0.60, 0.44, 0.26],
                secondary_rgb: [0.42, 0.29, 0.16],
                family: WoodGrain { fiber_freq: 4.0, contrast: 0.6 },
            },
            VoxelType::Pine => TextureRecipe {
                base_rgb: [0.78, 0.64, 0.42],
                secondary_rgb: [0.60, 0.46, 0.28],
                family: WoodGrain { fiber_freq: 3.0, contrast: 0.5 },
            },
        }
    }

    #[inline(always)]
    pub fn is_water(&self) -> bool {
        *self == VoxelType::Water
    }
}

pub fn generate_voxel_atlas(images: &mut Assets<Image>) -> Handle<Image> {
    let width = TILE_SIZE * ATLAS_TILES;
    let height = TILE_SIZE;
    let mut pixels = vec![255u8; width * height * 4];

    for voxel in VoxelType::ALL {
        let tile_idx = voxel.atlas_index();
        let recipe = voxel.recipe();
        let tau = std::f32::consts::TAU;

        for y in 0..TILE_SIZE {
            for x in 0..TILE_SIZE {
                let u = x as f32 / TILE_SIZE as f32;
                let v = y as f32 / TILE_SIZE as f32;
                let n = pixel_hash(x, y, tile_idx);

                // Calculate blend factor `t` in [0.0, 1.0] between base and secondary
                let t = match recipe.family {
                    PatternFamily::Solid => 0.0,
                    PatternFamily::Soil { contrast } => n * contrast,
                    PatternFamily::Stratified { band_freq, band_contrast, noise_contrast } => {
                        let wave = ((v * band_freq * tau + n * 0.8).sin() * 0.5 + 0.5) * band_contrast;
                        (wave + n * noise_contrast).clamp(0.0, 1.0)
                    }
                    PatternFamily::Speckled { density, contrast } => {
                        if n < density {
                            (n / density) * contrast
                        } else {
                            (n - 0.5) * 0.15
                        }
                    }
                    PatternFamily::FineGrain { contrast, glossy_streak } => {
                        let streak = if glossy_streak {
                            ((u + v * 1.5) * tau).sin().max(0.0).powf(4.0) * 0.6
                        } else {
                            0.0
                        };
                        (n * contrast + streak).clamp(0.0, 1.0)
                    }
                    PatternFamily::Foliated { vein_freq, sharpness, contrast } => {
                        let diag = (u + v + (n - 0.5) * 0.25) * vein_freq * tau;
                        let vein = (diag.sin() * 0.5 + 0.5).powf(sharpness);
                        (vein * contrast).clamp(0.0, 1.0)
                    }
                    PatternFamily::WoodGrain { fiber_freq, contrast } => {
                        let stretch_noise = pixel_hash(x, y / 4, tile_idx);
                        let grain = ((u * fiber_freq * tau + stretch_noise * 1.5).sin() * 0.5 + 0.5) * contrast;
                        (grain + n * 0.1).clamp(0.0, 1.0)
                    }
                };

                let t = t.clamp(0.0, 1.0);
                let r = recipe.base_rgb[0] * (1.0 - t) + recipe.secondary_rgb[0] * t;
                let g = recipe.base_rgb[1] * (1.0 - t) + recipe.secondary_rgb[1] * t;
                let b = recipe.base_rgb[2] * (1.0 - t) + recipe.secondary_rgb[2] * t;

                let px_idx = (y * width + (tile_idx * TILE_SIZE + x)) * 4;
                pixels[px_idx] = (r.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[px_idx + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[px_idx + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
                pixels[px_idx + 3] = 255;
            }
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );

    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });

    images.add(image)
}

fn pixel_hash(x: usize, y: usize, seed: usize) -> f32 {
    let mut n = (x.wrapping_mul(374761393))
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(982451653));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);

    ((n ^ (n >> 16)) & 0x7fffffff) as f32 / 2147483647.0
}

#[derive(Component)]
pub struct ChunkMeshMarker {
    pub cx: i32,
    pub cz: i32,
    pub min_y: usize,
    pub max_y: usize,
    pub current_rendered_y: usize,
}


#[derive(Clone)]
pub struct ChunkData {
    pub voxels: Box<[VoxelType; CHUNK_VOLUME]>,
}

impl Default for ChunkData {
    fn default() -> Self {
        Self {
            voxels: Box::new([VoxelType::Air; CHUNK_VOLUME]),
        }
    }
}

impl ChunkData {
    #[inline(always)]
    pub fn index(x: usize, y: usize, z: usize) -> usize {
        x + (y * CHUNK_SIZE) + (z * CHUNK_SIZE * CHUNK_SIZE)
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize, z: usize) -> VoxelType {
        self.voxels[Self::index(x, y, z)]
    }

    #[inline(always)]
    pub fn set(&mut self, x:usize, y:usize, z:usize, voxel: VoxelType) {
        self.voxels[Self::index(x, y, z)] = voxel;
    }
}

#[derive(Default)]
pub struct ChunkMeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tangents: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl ChunkMeshBuffers {
    pub fn push_face(&mut self, origin: [f32; 3], dir: Direction, voxel: VoxelType) {
        let base_idx = self.positions.len() as u32;
        let corners = dir.quad_vertices();
        let normal = dir.normal_f();
        let tangent = dir.tangent_f();

        // Emit 4 vertices
        for [vx, vy, vz] in corners {
            self.positions.push([origin[0] + vx, origin[1] + vy, origin[2] + vz]);
            self.normals.push(normal);
            self.tangents.push(tangent);
        }

        let tile = voxel.atlas_index() as f32;
        let total = ATLAS_TILES as f32;
        let eps = 0.001 / total; // prevent seam bleeding
        let u_min = (tile / total) + eps;
        let u_max = ((tile + 1.0) / total) - eps;

        self.uvs.extend_from_slice(&[
            [u_min, 1.0],
            [u_min, 0.0],
            [u_max, 0.0],
            [u_max, 1.0],
        ]);

        //Triangles that form quad
        self.indices.extend_from_slice(&[
            base_idx,
            base_idx + 1,
            base_idx + 2,
            base_idx,
            base_idx + 2,
            base_idx + 3,
        ]);
    }
}

//Build culled triangle mesh from chunk data buffer
pub fn generate_sliced_chunk_mesh(chunk: &ChunkData, max_render_y: usize) -> Mesh {
    let mut buffers = ChunkMeshBuffers::default();

    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                if y > max_render_y {
                    continue;
                }

                let current = chunk.get(x, y, z);
                if !current.is_opaque() {
                    continue;
                }

                let origin = [x as f32, y as f32, z as f32];

                for dir in DIRECTIONS {
                    if dir == Direction::PosY && y == max_render_y {
                        buffers.push_face(origin, dir, current);
                        continue;
                    }

                    let [dx, dy, dz] = dir.normal_i();
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let nz = z as i32 + dz;

                    let neighbor_opaque = if nx < 0 || nx >= CHUNK_SIZE as i32 || ny < 0 || ny >= CHUNK_SIZE as i32 || nz < 0 || nz >= CHUNK_SIZE as i32 {
                        false
                    } else {
                        chunk.get(nx as usize, ny as usize, nz as usize).is_opaque()
                    };

                    if !neighbor_opaque {
                        buffers.push_face(origin, dir, current);
                    }
                }
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffers.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, buffers.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, buffers.tangents);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, buffers.uvs);
    mesh.insert_indices(Indices::U32(buffers.indices));

    mesh
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    PosX, NegX,
    PosY, NegY,
    PosZ, NegZ,
}

pub const DIRECTIONS: [Direction; 6] = [
    Direction::PosX, Direction::NegX,
    Direction::PosY, Direction::NegY,
    Direction::PosZ, Direction::NegZ,
];

impl Direction {

    //Neighbor offset in (x, y, z)
    pub const fn normal_i(&self) -> [i32; 3] {
        match self {
            Direction::PosX => [1, 0, 0],
            Direction::NegX => [-1, 0, 0],
            Direction::PosY => [0, 1, 0],
            Direction::NegY => [0, -1, 0],
            Direction::PosZ => [0, 0, 1],
            Direction::NegZ => [0, 0, -1],
        }
    }

    pub const fn normal_f(&self) -> [f32; 3] {
        match self {
            Direction::PosX => [1.0, 0.0, 0.0],
            Direction::NegX => [-1.0, 0.0, 0.0],
            Direction::PosY => [0.0, 1.0, 0.0],
            Direction::NegY => [0.0, -1.0, 0.0],
            Direction::PosZ => [0.0, 0.0, 1.0],
            Direction::NegZ => [0.0, 0.0, -1.0],
        }
    }

    pub const fn tangent_f(&self) -> [f32; 4] {
        match self {
            Direction::PosX => [0.0, 0.0, -1.0, 1.0],
            Direction::NegX => [0.0, 0.0, 1.0, 1.0],
            Direction::PosY => [1.0, 0.0, 0.0, 1.0],
            Direction::NegY => [1.0, 0.0, 0.0, 1.0],
            Direction::PosZ => [-1.0, 0.0, 0.0, 1.0],
            Direction::NegZ => [1.0, 0.0, 0.0, 1.0],
        }
    }

    pub const fn quad_vertices(&self) -> [[f32; 3]; 4] {
        match self {
            //East (+X): Normal [1, 0, 0]
            Direction::PosX => [
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [1.0, 0.0, 1.0],
            ],

            //West (-X): Normal [-1, 0, 0]
            Direction::NegX => [
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],

            //Top (+Y): Normal [0, 1, 0]
            Direction::PosY => [
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],

            //Bottom (-Y): Normal [0, -1, 0]
            Direction::NegY => [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],

            //South (+Z): Normal [0, 0, 1]
            Direction::PosZ => [
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
                [0.0, 0.0, 1.0],
            ],

            //North (-Z): Normal [0, 0, -1]
            Direction::NegZ => [
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
            ],
        }
    }
}