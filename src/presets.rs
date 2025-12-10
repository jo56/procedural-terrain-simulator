use serde::{Deserialize, Serialize};

use crate::particles::ParticleSettings;
use crate::sky::{SkySettings, DEFAULT_MOON_PARALLAX};
use crate::terrain::{TerrainSettings, DEFAULT_FOG_START, DEFAULT_FOG_DISTANCE};

// Preset-specific ambient value (differs from TerrainSettings::default() which uses 0.25)
const PRESET_AMBIENT: f32 = 0.35;
pub const DEFAULT_PRESET_ID: &str = "arctic";

// Preset-specific sky object parameters (differ from SkySettings::default())
const PRESET_STAR_SIZE_MIN: f32 = 0.3;
const PRESET_STAR_SIZE_MAX: f32 = 1.5;
const PRESET_STAR_TWINKLE_SPEED: f32 = 1.0;
const PRESET_STAR_PARALLAX: f32 = 0.1;
const PRESET_SUN_SIZE: f32 = 60.0;
const PRESET_SUN_PARALLAX: f32 = 0.05;
const PRESET_MOON_SIZE: f32 = 45.0;

/// A complete preset containing all settings for terrain, sky, and particles
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FullPreset {
    pub name: String,
    pub terrain: TerrainSettings,
    pub sky: SkySettings,
    pub particles: ParticleSettings,
}

/// Metadata about a preset (for listing without full data)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
}

/// Get list of available preset IDs and names
pub fn get_preset_list() -> Vec<PresetInfo> {
    vec![
        PresetInfo {
            id: "arctic".to_string(),
            name: "Arctic".to_string(),
        },
        PresetInfo {
            id: "chalk".to_string(),
            name: "Moon".to_string(),
        },
        PresetInfo {
            id: "desert".to_string(),
            name: "Desert".to_string(),
        },
        PresetInfo {
            id: "lava".to_string(),
            name: "Lava".to_string(),
        },
        PresetInfo {
            id: "natural".to_string(),
            name: "Islands".to_string(),
        },
    ]
}

/// Get a full preset by ID
pub fn get_preset(id: &str) -> Option<FullPreset> {
    match id {
        "chalk" => Some(chalk_preset()),
        "natural" => Some(natural_preset()),
        "desert" => Some(desert_preset()),
        "lava" => Some(lava_preset()),
        "arctic" => Some(arctic_preset()),
        _ => None,
    }
}

/// Get the default preset ID
pub fn get_default_preset_id() -> &'static str {
    DEFAULT_PRESET_ID
}

/// Convenience helper for retrieving the default preset
pub fn get_default_preset() -> Option<FullPreset> {
    get_preset(DEFAULT_PRESET_ID)
}

fn chalk_preset() -> FullPreset {
    // This uses the current Default implementations
    FullPreset {
        name: "Night".to_string(),
        terrain: TerrainSettings::default(),
        sky: SkySettings::default(),
        particles: ParticleSettings::default(),
    }
}

fn natural_preset() -> FullPreset {
    FullPreset {
        name: "Day".to_string(),
        terrain: TerrainSettings {
            terrain_scale: 0.001,
            height_scale: 1501.0,
            octaves: 4,
            warp_strength: 1.0,
            height_variance: 0.6,
            roughness: 0.84,
            pattern_type: 2, // Islands pattern
            seed: 0,
            ambient: PRESET_AMBIENT,
            fog_start: DEFAULT_FOG_START,
            fog_distance: DEFAULT_FOG_DISTANCE,
            // Natural colors
            color_abyss: [0.05, 0.1, 0.25],
            color_deep_water: [0.1, 0.2, 0.4],
            color_shallow_water: [0.2, 0.4, 0.6],
            color_sand: [0.76, 0.7, 0.5],
            color_grass: [0.22, 0.45, 0.15],
            color_rock: [0.45, 0.42, 0.38],
            color_snow: [0.95, 0.95, 0.98],
            // Sky colors - bright blue
            color_sky: [0.53, 0.81, 0.92],
            color_sky_top: [0.25, 0.5, 0.8],
            color_sky_horizon: [0.75, 0.85, 0.95],
        },
        sky: SkySettings {
            star_count: 0,
            star_size_min: PRESET_STAR_SIZE_MIN,
            star_size_max: PRESET_STAR_SIZE_MAX,
            star_color: [1.0, 1.0, 0.9],
            star_twinkle_speed: PRESET_STAR_TWINKLE_SPEED,
            star_parallax: PRESET_STAR_PARALLAX,
            sun_count: 0,
            sun_size: PRESET_SUN_SIZE,
            sun_color: [1.0, 0.95, 0.8],
            sun_parallax: PRESET_SUN_PARALLAX,
            moon_count: 0,
            moon_size: PRESET_MOON_SIZE,
            moon_color: [0.9, 0.9, 0.95],
            moon_parallax: DEFAULT_MOON_PARALLAX,
            seed: 0,
        },
        particles: ParticleSettings::default(), // No weather by default
    }
}

fn desert_preset() -> FullPreset {
    FullPreset {
        name: "Desert".to_string(),
        terrain: TerrainSettings {
            terrain_scale: 0.001,
            height_scale: 511.0,
            octaves: 1,
            warp_strength: 30.0,
            height_variance: 0.30,
            roughness: 0.50,
            pattern_type: 3, // Valleys
            seed: 0,
            ambient: PRESET_AMBIENT,
            fog_start: DEFAULT_FOG_START,
            fog_distance: DEFAULT_FOG_DISTANCE,
            // Desert theme colors
            color_abyss: [0.08, 0.05, 0.02],
            color_deep_water: [0.15, 0.1, 0.05],
            color_shallow_water: [0.35, 0.25, 0.15],
            color_sand: [0.85, 0.78, 0.63],
            color_grass: [0.7, 0.6, 0.4],
            color_rock: [0.55, 0.45, 0.35],
            color_snow: [0.95, 0.9, 0.85],
            color_sky: [0.65, 0.55, 0.45],
            color_sky_top: [0.45, 0.35, 0.25],
            color_sky_horizon: [0.95, 0.85, 0.7],
        },
        sky: SkySettings {
            star_count: 500,
            star_size_min: PRESET_STAR_SIZE_MIN,
            star_size_max: PRESET_STAR_SIZE_MAX,
            star_color: [1.0, 0.95, 0.8],
            star_twinkle_speed: PRESET_STAR_TWINKLE_SPEED,
            star_parallax: PRESET_STAR_PARALLAX,
            sun_count: 0,
            sun_size: PRESET_SUN_SIZE,
            sun_color: [1.0, 0.85, 0.5],
            sun_parallax: PRESET_SUN_PARALLAX,
            moon_count: 0,
            moon_size: PRESET_MOON_SIZE,
            moon_color: [0.95, 0.9, 0.8],
            moon_parallax: DEFAULT_MOON_PARALLAX,
            seed: 0,
        },
        particles: ParticleSettings::default(), // No weather
    }
}

fn lava_preset() -> FullPreset {
    FullPreset {
        name: "Lava".to_string(),
        terrain: TerrainSettings {
            terrain_scale: 0.001,
            height_scale: 436.0,
            octaves: 2,
            warp_strength: 5.0,
            height_variance: 0.15,
            roughness: 0.22,
            pattern_type: 1, // Ridged
            seed: 0,
            ambient: PRESET_AMBIENT,
            fog_start: DEFAULT_FOG_START,
            fog_distance: DEFAULT_FOG_DISTANCE,
            // Volcanic theme colors
            color_abyss: [0.05, 0.0, 0.0],
            color_deep_water: [0.2, 0.02, 0.0],
            color_shallow_water: [0.6, 0.15, 0.0],
            color_sand: [0.15, 0.12, 0.1],
            color_grass: [0.25, 0.18, 0.12],
            color_rock: [0.35, 0.25, 0.2],
            color_snow: [0.5, 0.4, 0.35],
            color_sky: [0.15, 0.05, 0.02],
            color_sky_top: [0.08, 0.02, 0.01],
            color_sky_horizon: [0.3, 0.1, 0.02],
        },
        sky: SkySettings {
            star_count: 1000,
            star_size_min: PRESET_STAR_SIZE_MIN,
            star_size_max: PRESET_STAR_SIZE_MAX,
            star_color: [1.0, 0.6, 0.2],
            star_twinkle_speed: PRESET_STAR_TWINKLE_SPEED,
            star_parallax: PRESET_STAR_PARALLAX,
            sun_count: 10,
            sun_size: PRESET_SUN_SIZE,
            sun_color: [1.0, 0.4, 0.1],
            sun_parallax: PRESET_SUN_PARALLAX,
            moon_count: 0,
            moon_size: PRESET_MOON_SIZE,
            moon_color: [0.8, 0.3, 0.1],
            moon_parallax: DEFAULT_MOON_PARALLAX,
            seed: 0,
        },
        particles: ParticleSettings::default(), // No weather
    }
}

fn arctic_preset() -> FullPreset {
    FullPreset {
        name: "Arctic".to_string(),
        terrain: TerrainSettings {
            terrain_scale: 0.008,
            height_scale: 611.0,
            octaves: 5,
            warp_strength: 1.0,
            height_variance: 0.10,
            roughness: 0.22,
            pattern_type: 3, // Valleys
            seed: 0,
            ambient: PRESET_AMBIENT,
            fog_start: DEFAULT_FOG_START,
            fog_distance: DEFAULT_FOG_DISTANCE,
            // Arctic colors
            color_abyss: [0.02, 0.08, 0.15],
            color_deep_water: [0.05, 0.15, 0.25],
            color_shallow_water: [0.2, 0.35, 0.5],
            color_sand: [0.7, 0.75, 0.8],
            color_grass: [0.5, 0.55, 0.5],
            color_rock: [0.4, 0.42, 0.45],
            color_snow: [0.98, 0.98, 1.0],
            color_sky: [0.75, 0.85, 0.95],
            color_sky_top: [0.5, 0.65, 0.85],
            color_sky_horizon: [0.85, 0.9, 0.98],
        },
        sky: SkySettings {
            star_count: 0,
            star_size_min: PRESET_STAR_SIZE_MIN,
            star_size_max: PRESET_STAR_SIZE_MAX,
            star_color: [0.9, 0.95, 1.0], // Arctic star color
            star_twinkle_speed: PRESET_STAR_TWINKLE_SPEED,
            star_parallax: PRESET_STAR_PARALLAX,
            sun_count: 0,
            sun_size: PRESET_SUN_SIZE,
            sun_color: [1.0, 0.98, 0.95], // Arctic sun color
            sun_parallax: PRESET_SUN_PARALLAX,
            moon_count: 0,
            moon_size: PRESET_MOON_SIZE,
            moon_color: [0.85, 0.9, 1.0], // Arctic moon color
            moon_parallax: DEFAULT_MOON_PARALLAX,
            seed: 0,
        },
        particles: ParticleSettings::default(), // No particles
    }
}
