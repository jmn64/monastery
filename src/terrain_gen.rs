use bevy::prelude::*;
use crate::voxel::VoxelType;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RockCategory {
    Sedimentary,
    IgneousIntrusive,
    IgneousExtrusive,
    Metamorphic,
}

pub struct Heightfield2d {
    pub size: usize,
    pub height: Vec<f32>,
    pub bedrock: Vec<RockCategory>,
}

impl Heightfield2d {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            height: vec![0.0; size * size],
            bedrock: vec![RockCategory::Sedimentary; size * size],
        }
    }

    #[inline(always)]
    pub fn index(&self, x: usize, z: usize) -> usize {
        x + (z * self.size)
    }

    pub fn get_height(&self, x: usize, z: usize) -> f32 {
        self.height[self.index(x, z)]
    }

    pub fn set_height(&mut self, x: usize, z: usize, val: f32) {
        let idx = self.index(x, z);
        self.height[idx] = val;
    }
}

fn hash2d(x: i32, z: i32) -> f32 {
    let mut n = (x.wrapping_mul(374761393)).wrapping_add(z.wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n ^ (n >> 16)) as f32 / 2147483648.0
}

fn smooth_noise(x: f32, z: f32) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - x.floor();
    let fz = z - z.floor();

    // smoothstep
    let wx = fx * fx * (3.0 - 2.0 * fx);
    let wz = fz * fz * (3.0 - 2.0 * fz);

    let v00 = hash2d(ix, iz);
    let v10 = hash2d(ix + 1, iz);
    let v01 = hash2d(ix, iz + 1);
    let v11 = hash2d(ix + 1, iz + 1);

    let x0 = v00 * (1.0 - wx) + v10 * wx;
    let x1 = v01 * (1.0 - wx) + v11 * wx;

    x0 * (1.0 - wz) + x1 * wz
}

pub fn generate_base_noise(field: &mut Heightfield2d, min_height: f32, max_height: f32) {
    let size = field.size;
    let octaves = 4;
    let base_scale = 0.015;

    for z in 0..size {
        for x in 0..size {
            let mut amp = 1.0;
            let mut freq = base_scale;
            let mut val = 0.0;
            let mut total_amp = 0.0;

            for _ in 0..octaves {
                val += smooth_noise(x as f32 * freq, z as f32 * freq) * amp;
                total_amp += amp;
                amp *= 0.5;
                freq *= 2.0;
            }

            let normalized = (val / total_amp).clamp(0.0, 1.0);
            let h = min_height + normalized * (max_height - min_height);
            field.set_height(x, z, h);

            // assign rock categories
            let idx = field.index(x, z);
            field.bedrock[idx] = if h > (min_height + (max_height - min_height) * 0.75) {
                RockCategory::IgneousIntrusive
            } else if h > (min_height + (max_height - min_height) * 0.45) {
                RockCategory::Metamorphic
            } else {
                RockCategory::Sedimentary
            }
        }
    }
}

// Hydraulic Erosion

#[derive(Resource, Clone, Copy, Debug)]
pub struct ErosionSettings {
    pub droplets: usize,
    pub inertia: f32,
    pub sediment_capacity: f32,
    pub min_sediment_capacity: f32,
    pub deposition_speed: f32,
    pub erosion_speed: f32,
    pub evaporation_speed: f32,
    pub max_steps: usize,
}

impl Default for ErosionSettings {
    fn default() -> Self {
        Self {
            droplets: 40_000,
            inertia: 0.1,
            sediment_capacity: 4.0,
            min_sediment_capacity: 0.05,
            deposition_speed: 0.1,
            erosion_speed: 0.1,
            evaporation_speed: 0.02,
            max_steps: 30,
        }
    }
}

pub fn apply_hydraulic_erosion(field: &mut Heightfield2d, settings: &ErosionSettings) {
    let size = field.size;
    let max_coord = (size - 2) as f32;
    let mut rng: u32 = 424242;

    let mut rand_f = || {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        (rng >> 16) as f32 / 65535.0
    };

    for _ in 0..settings.droplets {
        let mut px = rand_f() * (max_coord - 1.0) + 1.0;
        let mut pz = rand_f() * (max_coord - 1.0) + 1.0;
        let mut dir_x = 0.0;
        let mut dir_z = 0.0;
        let mut speed = 1.0;
        let mut water = 1.0;
        let mut sediment = 0.0;

        for _ in 0..settings.max_steps {
            let ix = px as usize;
            let iz = pz as usize;

            let h_curr = field.get_height(ix, iz);
            let grad_x = field.get_height(ix + 1, iz) - h_curr;
            let grad_z = field.get_height(ix, iz + 1) - h_curr;

            dir_x = (dir_x * settings.inertia) - (grad_x * (1.0 - settings.inertia));
            dir_z = (dir_z * settings.inertia) - (grad_z * (1.0 - settings.inertia));

            let len = (dir_x * dir_x + dir_z * dir_z).sqrt();
            if len == 0.0 {
                break;
            }
            dir_x /= len;
            dir_z /= len;

            let next_px = px + dir_x;
            let next_pz = pz + dir_z;

            if next_px < 1.0 || next_px >= max_coord || next_pz < 1.0 || next_pz >= max_coord {
                break;
            }

            let h_next = field.get_height(next_px as usize, next_pz as usize);
            let dh = h_next - h_curr;

            let c = ((-dh).max(0.0) * speed * water * settings.sediment_capacity).max(settings.min_sediment_capacity);

            if sediment > c || dh > 0.0 {
                let deposit = if dh > 0.0 {
                    sediment.min(dh)
                } else {
                    (sediment - c) * settings.deposition_speed
                };

                sediment -= deposit;
                field.set_height(ix, iz, h_curr + deposit);
            } else {
                let erode = ((c - sediment) * settings.erosion_speed).min(-dh);
                sediment += erode;
                field.set_height(ix, iz, h_curr - erode);
            }

            speed = (speed * speed + dh.abs() * 9.8).sqrt();
            water *= 1.0 - settings.evaporation_speed;
            px = next_px;
            pz = next_pz;
        }
    }
}