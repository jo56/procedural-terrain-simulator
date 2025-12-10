// Terrain generation and rendering settings
export interface TerrainSettings {
    terrain_scale: number;
    height_scale: number;
    octaves: number;
    warp_strength: number;
    height_variance: number;
    roughness: number;
    pattern_type: number;
    seed: number;
    ambient: number;
    fog_start: number;
    fog_distance: number;
    color_abyss: [number, number, number];
    color_deep_water: [number, number, number];
    color_shallow_water: [number, number, number];
    color_sand: [number, number, number];
    color_grass: [number, number, number];
    color_rock: [number, number, number];
    color_snow: [number, number, number];
    color_sky: [number, number, number];
    color_sky_top: [number, number, number];
    color_sky_horizon: [number, number, number];
}

// Color theme for terrain and sky
export interface ColorTheme {
    abyss: [number, number, number];
    deep_water: [number, number, number];
    shallow_water: [number, number, number];
    sand: [number, number, number];
    grass: [number, number, number];
    rock: [number, number, number];
    snow: [number, number, number];
    sky: [number, number, number];
    sky_top: [number, number, number];
    sky_horizon: [number, number, number];
    star_color: [number, number, number];
    sun_color: [number, number, number];
    moon_color: [number, number, number];
}

// Sky rendering settings
export interface SkySettings {
    star_count: number;
    star_size_min: number;
    star_size_max: number;
    star_color: [number, number, number];
    star_twinkle_speed: number;
    star_parallax: number;
    sun_count: number;
    sun_size: number;
    sun_color: [number, number, number];
    sun_parallax: number;
    moon_count: number;
    moon_size: number;
    moon_color: [number, number, number];
    moon_parallax: number;
    seed: number;
}

// Weather particle system settings
export interface ParticleSettings {
    particle_type: number;       // 0=rain, 1=snow
    density: number;
    max_particles: number;
    speed: number;
    wind_x: number;
    wind_z: number;
    particle_size: number;
    particle_color: [number, number, number, number];
    spawn_height: number;
    spawn_radius: number;
}

// Preset metadata
export interface PresetInfo {
    id: string;
    name: string;
}

// Full preset containing all settings
export interface FullPreset {
    name: string;
    terrain: TerrainSettings;
    sky: SkySettings;
    particles: ParticleSettings;
}

// Slider scaling configuration for non-linear controls
export interface SliderConfig {
    min: number;
    max: number;
    exponent: number;  // Higher = more compression at low end
    decimals: number;  // For display formatting
}

// Slider setup configuration
export interface SliderSetup {
    id: string;
    decimals?: number;
}

// Helper type aliases for color input groupings
export type TerrainColorInputs = Pick<TerrainSettings, 'color_abyss' | 'color_deep_water' | 'color_shallow_water' | 'color_sand' | 'color_grass' | 'color_rock' | 'color_snow' | 'color_sky' | 'color_sky_top' | 'color_sky_horizon'>;
export type SkyColorInputs = Pick<SkySettings, 'star_color' | 'sun_color' | 'moon_color'>;

// Change category for settings comparison
export type ChangeCategory = 'none' | 'colors_only' | 'generation';
