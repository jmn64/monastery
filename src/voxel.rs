use std::collections::HashMap;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use bevy::tasks::ComputeTaskPool;
use crate::camera::{CurrentLevel, SliceSettings};
use crate::terrain_gen::{apply_hydraulic_erosion, generate_base_noise, ErosionSettings, Heightfield2d};

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
    mut chunk_storage: ResMut<WorldChunkStorage>,
    level: Res<CurrentLevel>,
    slice_settings: Res<SliceSettings>,
) {
    let material_handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });

    // allocate heightfield for world bounds
    let mut heightfield = Heightfield2d::new(MAP_DIMENSION);

    // Multi pass generation: noise base + hydraulic
    generate_base_noise(&mut heightfield, 6.0, (CHUNK_SIZE - 4) as f32);
    apply_hydraulic_erosion(&mut heightfield, &ErosionSettings::default());

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

            for ly in 0..CHUNK_SIZE {
                let voxel = if ly < ground_height.saturating_sub(3) {
                    VoxelType::Stone
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

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VoxelType {
    Air = 0,
    Stone = 1,
    Dirt = 2,
    Grass = 3,
    Water = 4,
}

impl VoxelType {
    #[inline(always)]
    pub fn is_opaque(&self) -> bool {
        *self != VoxelType::Air
    }

    #[inline(always)]
    pub fn is_water(&self) -> bool {
        *self == VoxelType::Water
    }

    pub fn color(&self) -> [f32; 4] {
        match self {
            VoxelType::Air => [0.0, 0.0, 0.0, 0.0],
            VoxelType::Stone => [0.07, 0.07, 0.06, 1.0],
            VoxelType::Dirt => [0.50, 0.52, 0.55, 1.0],
            VoxelType::Grass => [0.28, 0.50, 0.22, 1.0],
            VoxelType::Water => [0.15, 0.35, 0.75, 0.75],
        }
    }
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
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl ChunkMeshBuffers {
    pub fn push_face(&mut self, origin: [f32; 3], dir: Direction, color: [f32; 4]) {
        let base_idx = self.positions.len() as u32;
        let corners = dir.quad_vertices();
        let normal = dir.normal_f();

        // Emit 4 vertices
        for [vx, vy, vz] in corners {
            self.positions.push([origin[0] + vx, origin[1] + vy, origin[2] + vz]);
            self.normals.push(normal);
            self.colors.push(color);
        }

        self.uvs.extend_from_slice(&[
            [0.0, 1.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
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
                        buffers.push_face(origin, dir, current.color());
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
                        buffers.push_face(origin, dir, current.color());
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, buffers.colors);
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