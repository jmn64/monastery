use std::collections::HashMap;
use bevy::prelude::*;
use crate::environmental::Temperature;

#[derive(Component)]
struct Tree {
    species: TreeSpecies,
    age: u32, // in days
    local_voxels: HashMap<IVec3, TreeCellType>,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum TreeSpecies {
    Pine,
    Oak,
    Cyprus,
    Chestnut,
    Olive,
    Apple,
    Fig,
    Pear,
    Cherry,
    Bay,
    Mulberry,
}
pub enum TreeCellType {
    Empty,
    Trunk,
    Branch,
    Leaf,
    TapRoot,
    BranchRoot,
}

#[derive(Resource)]
pub struct SpeciesConfigs {
    pub map: HashMap<TreeSpecies, GrowthParams>,
}

pub struct GrowthParams {
    pub max_height: u32,
    pub max_root_depth: u32,
    pub root_branch_probability: f32,
    pub max_root_extension: u8,
    pub max_leaf_extension: u8,
    pub branch_probability: f32,
    pub vertical_growth_bias: f32, // Higher = straighter
    pub min_temperature: Temperature,
    pub max_temperature: Temperature,
}