use bevy::prelude::*;
use bevy::render::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

pub fn spawn_voxel_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let material_handle = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.85,
        ..default()
    });

    const GRID_RADIUS: i32 = 8;

    for cx in -GRID_RADIUS..GRID_RADIUS {
        for cz in -GRID_RADIUS..GRID_RADIUS {
            let chunk_data = generate_terrain_chunk(cx, cz);
            let mesh = generate_chunk_mesh(&chunk_data);

            let world_x = (cx * CHUNK_SIZE as i32) as f32;
            let world_z = (cz * CHUNK_SIZE as i32) as f32;

            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material_handle.clone()),
                Transform::from_xyz(world_x, 0.0, world_z),
                ));
        }
    }
}

pub fn generate_terrain_chunk(chunk_x: i32, chunk_z: i32) -> ChunkData {
    let mut chunk = ChunkData::default();

    for lx in 0..CHUNK_SIZE {
        for lz in 0..CHUNK_SIZE {
            let wx = (chunk_x * CHUNK_SIZE as i32) + lx as i32;
            let wz = (chunk_z * CHUNK_SIZE as i32) + lz as i32;
            let height = sample_terrain_height(wx, wz);

            for ly in 0..CHUNK_SIZE {
                let voxel = if ly < height.saturating_sub(2) {
                    VoxelType::Stone
                } else if ly <= height {
                    VoxelType::Dirt
                } else {
                    VoxelType::Air
                };
                chunk.set(lx, ly, lz, voxel);
            }
        }
    }

    chunk
}

pub fn sample_terrain_height(world_x: i32, world_z: i32) -> usize {
    let fx = world_x as f32 * 0.08;
    let fz = world_z as f32 * 0.08;

    // base height 14, ranging 8 to 20
    let elevation = 14.0 + (fx.sin() * 4.0) + (fz.cos() * 3.0) + ((fx + fz).sin() * 2.0);

    elevation.clamp(1.0, (CHUNK_SIZE - 2) as f32) as usize
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VoxelType {
    Air = 0,
    Stone = 1,
    Dirt = 2,
}

impl VoxelType {
    #[inline(always)]
    pub fn is_opaque(&self) -> bool {
        *self != VoxelType::Air
    }

    pub fn color(&self) -> [f32; 4] {
        match self {
            VoxelType::Air => [0.0, 0.0, 0.0, 0.0],
            VoxelType::Stone => [0.07, 0.07, 0.06, 1.0],
            VoxelType::Dirt => [0.50, 0.52, 0.55, 1.0],
        }
    }
}

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
pub fn generate_chunk_mesh(chunk: &ChunkData) -> Mesh {
    let mut buffers = ChunkMeshBuffers::default();

    for  x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let current = chunk.get(x, y, z);
                if !current.is_opaque() {
                    continue;
                }

                let origin = [x as f32, y as f32, z as f32];

                for dir in DIRECTIONS {
                    let [dx, dy, dz] = dir.normal_i();
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    let nz = z as i32 + dz;

                    // If outside the chunk bounds, treat as air and render boundary face
                    let neighbor_opaque = if nx < 0 || nx >= CHUNK_SIZE as i32
                        || ny < 0 || ny >= CHUNK_SIZE as i32
                        || nz < 0 || nz >= CHUNK_SIZE as i32
                    {
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

    // Convert buffers into GPU mesh structure
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD, // free from CPU RAM after uploading to VRAM
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