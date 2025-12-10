import type { ColorTheme, SkySettings, ParticleSettings, SliderConfig, SliderSetup, TerrainSettings } from './types';

// Comparison tolerances for color and number equality checks
export const COLOR_EPSILON = 0.005;
export const NUMBER_EPSILON = 0.0001;

// Seed generation max value
export const SEED_MAX = 1000000;

// Snow-type weather presets for particle type detection
export const SNOW_WEATHER_TYPES = new Set(['light-snow', 'heavy-snow', 'blizzard']);

// Non-linear scaling for sliders to provide more granularity at lower values
export const SLIDER_CONFIGS: Record<string, SliderConfig> = {
    'terrain-scale': { min: 0.0005, max: 0.05, exponent: 3.0, decimals: 4 },
    'warp-strength': { min: 0, max: 100, exponent: 2.0, decimals: 0 },
    'roughness': { min: 0.1, max: 0.9, exponent: 1.5, decimals: 2 },
};

// Configuration for slider setup (used in setupSettingsPanel)
export const SLIDER_SETUP: SliderSetup[] = [
    { id: 'terrain-scale' },
    { id: 'height-scale' },
    { id: 'height-variance', decimals: 2 },
    { id: 'octaves' },
    { id: 'warp-strength' },
    { id: 'roughness' },
    { id: 'star-count' },
    { id: 'sun-count' },
    { id: 'moon-count' },
    { id: 'particle-density', decimals: 1 },
    { id: 'particle-speed' },
    { id: 'wind-x' },
    { id: 'wind-z' },
];

/**
 * Color Themes - Quick color palette swaps for terrain and sky.
 *
 * These are distinct from Rust presets (which include full terrain/sky/particle configs).
 * Color themes only change visual colors without affecting terrain generation parameters.
 *
 * Some themes overlap with Rust presets but serve different use cases:
 * - Presets: Load complete environment configurations
 * - Color Themes: Swap colors while keeping current generation settings
 */
export const COLOR_THEMES: Record<string, ColorTheme> = {
    natural: {
        abyss: [0.05, 0.1, 0.25],
        deep_water: [0.1, 0.2, 0.4],
        shallow_water: [0.2, 0.4, 0.6],
        sand: [0.76, 0.7, 0.5],
        grass: [0.22, 0.45, 0.15],
        rock: [0.45, 0.42, 0.38],
        snow: [0.95, 0.95, 0.98],
        sky: [0.6, 0.7, 0.85],
        sky_top: [0.4, 0.55, 0.8],
        sky_horizon: [0.75, 0.82, 0.92],
        star_color: [1.0, 1.0, 0.9],
        sun_color: [1.0, 0.95, 0.8],
        moon_color: [0.9, 0.9, 0.95],
    },
    desert: {
        abyss: [0.08, 0.05, 0.02],
        deep_water: [0.15, 0.1, 0.05],
        shallow_water: [0.35, 0.25, 0.15],
        sand: [0.9, 0.8, 0.6],
        grass: [0.7, 0.6, 0.4],
        rock: [0.55, 0.45, 0.35],
        snow: [0.95, 0.9, 0.85],
        sky: [0.85, 0.75, 0.6],
        sky_top: [0.6, 0.5, 0.35],
        sky_horizon: [0.95, 0.85, 0.7],
        star_color: [1.0, 0.95, 0.8],
        sun_color: [1.0, 0.85, 0.5],
        moon_color: [0.95, 0.9, 0.8],
    },
    arctic: {
        abyss: [0.02, 0.08, 0.15],
        deep_water: [0.05, 0.15, 0.25],
        shallow_water: [0.2, 0.35, 0.5],
        sand: [0.7, 0.75, 0.8],
        grass: [0.5, 0.55, 0.5],
        rock: [0.4, 0.42, 0.45],
        snow: [0.98, 0.98, 1.0],
        sky: [0.75, 0.85, 0.95],
        sky_top: [0.5, 0.65, 0.85],
        sky_horizon: [0.85, 0.9, 0.98],
        star_color: [0.9, 0.95, 1.0],
        sun_color: [1.0, 0.98, 0.95],
        moon_color: [0.85, 0.9, 1.0],
    },
    alien: {
        abyss: [0.05, 0.02, 0.1],
        deep_water: [0.1, 0.05, 0.2],
        shallow_water: [0.2, 0.1, 0.35],
        sand: [0.4, 0.5, 0.3],
        grass: [0.1, 0.5, 0.4],
        rock: [0.3, 0.25, 0.4],
        snow: [0.7, 0.9, 0.85],
        sky: [0.5, 0.35, 0.6],
        sky_top: [0.3, 0.15, 0.5],
        sky_horizon: [0.6, 0.45, 0.7],
        star_color: [0.8, 0.5, 1.0],
        sun_color: [0.3, 1.0, 0.5],
        moon_color: [0.7, 0.3, 0.9],
    },
    volcanic: {
        abyss: [0.02, 0.01, 0.01],
        deep_water: [0.05, 0.02, 0.02],
        shallow_water: [0.15, 0.05, 0.02],
        sand: [0.25, 0.15, 0.1],
        grass: [0.2, 0.15, 0.1],
        rock: [0.15, 0.12, 0.1],
        snow: [0.9, 0.4, 0.1],
        sky: [0.3, 0.2, 0.15],
        sky_top: [0.15, 0.08, 0.05],
        sky_horizon: [0.5, 0.3, 0.2],
        star_color: [1.0, 0.6, 0.3],
        sun_color: [1.0, 0.4, 0.1],
        moon_color: [0.8, 0.3, 0.2],
    },
    ink: {
        abyss: [0.02, 0.02, 0.02],
        deep_water: [0.05, 0.05, 0.05],
        shallow_water: [0.15, 0.15, 0.15],
        sand: [0.25, 0.25, 0.25],
        grass: [0.12, 0.12, 0.12],
        rock: [0.3, 0.3, 0.3],
        snow: [0.08, 0.08, 0.08],
        sky: [0.76, 0.76, 0.76],
        sky_top: [0.6, 0.6, 0.6],
        sky_horizon: [0.85, 0.85, 0.85],
        star_color: [0.1, 0.1, 0.1],
        sun_color: [0.15, 0.15, 0.15],
        moon_color: [0.08, 0.08, 0.08],
    },
    chalk: {
        abyss: [0.4, 0.4, 0.4],
        deep_water: [0.6, 0.6, 0.6],
        shallow_water: [0.7, 0.7, 0.7],
        sand: [0.85, 0.85, 0.85],
        grass: [0.75, 0.75, 0.75],
        rock: [0.9, 0.9, 0.9],
        snow: [0.98, 0.98, 0.98],
        sky: [0.05, 0.05, 0.05],
        sky_top: [0.02, 0.02, 0.02],
        sky_horizon: [0.15, 0.15, 0.15],
        star_color: [0.95, 0.95, 0.95],
        sun_color: [1.0, 1.0, 1.0],
        moon_color: [0.9, 0.9, 0.9],
    },
};

export const SKY_PRESETS: Record<string, Partial<SkySettings>> = {
    'none': {
        star_count: 0,
        sun_count: 0,
        moon_count: 0,
    },
    'starry-night': {
        star_count: 300,
        star_size_min: 0.5,
        star_size_max: 2.5,
        star_color: [1.0, 1.0, 0.9],
        star_twinkle_speed: 1.0,
        star_parallax: 0.1,
        sun_count: 0,
        moon_count: 1,
        moon_size: 40.0,
        moon_color: [0.9, 0.9, 0.95],
        moon_parallax: 0.08,
    },
    'dual-suns': {
        star_count: 50,
        star_color: [1.0, 0.9, 0.7],
        sun_count: 2,
        sun_size: 60.0,
        sun_color: [1.0, 0.85, 0.6],
        sun_parallax: 0.05,
        moon_count: 0,
    },
    'full-moon': {
        star_count: 200,
        star_size_min: 0.3,
        star_size_max: 1.5,
        star_color: [0.9, 0.95, 1.0],
        sun_count: 0,
        moon_count: 1,
        moon_size: 80.0,
        moon_color: [0.95, 0.95, 1.0],
        moon_parallax: 0.06,
    },
    'clear-day': {
        star_count: 0,
        sun_count: 1,
        sun_size: 50.0,
        sun_color: [1.0, 0.95, 0.8],
        sun_parallax: 0.04,
        moon_count: 0,
    },
};

export const WEATHER_PRESETS: Record<string, Partial<ParticleSettings>> = {
    'none': {
        density: 0,
    },
    'light-rain': {
        particle_type: 0,
        density: 0.3,
        speed: 30,
        wind_x: 2,
        wind_z: 1,
        particle_size: 0.4,
        particle_color: [0.7, 0.8, 0.9, 0.5],
    },
    'heavy-rain': {
        particle_type: 0,
        density: 0.5,
        speed: 40,
        wind_x: 3,
        wind_z: 1,
        particle_size: 0.5,
        particle_color: [0.6, 0.7, 0.85, 0.6],
    },
    'drizzle': {
        particle_type: 0,
        density: 0.2,
        speed: 20,
        wind_x: 1,
        wind_z: 0.5,
        particle_size: 0.3,
        particle_color: [0.75, 0.8, 0.9, 0.4],
    },
    'light-snow': {
        particle_type: 1,
        density: 0.3,
        speed: 5,
        wind_x: 1,
        wind_z: 0.5,
        particle_size: 0.4,
        particle_color: [1.0, 1.0, 1.0, 0.7],
    },
    'heavy-snow': {
        particle_type: 1,
        density: 0.5,
        speed: 8,
        wind_x: 2,
        wind_z: 1,
        particle_size: 0.5,
        particle_color: [0.95, 0.95, 1.0, 0.75],
    },
    'blizzard': {
        particle_type: 1,
        density: 0.7,
        speed: 12,
        wind_x: 5,
        wind_z: 3,
        particle_size: 0.5,
        particle_color: [0.9, 0.92, 0.98, 0.8],
    },
};

// Generation settings that require terrain regeneration
export const GENERATION_SETTINGS: (keyof TerrainSettings)[] = [
    'pattern_type',
    'terrain_scale',
    'height_scale',
    'height_variance',
    'octaves',
    'warp_strength',
    'roughness',
];

// Color settings that only require update (no regeneration)
export const COLOR_SETTINGS: (keyof TerrainSettings)[] = [
    'color_abyss',
    'color_deep_water',
    'color_shallow_water',
    'color_sand',
    'color_grass',
    'color_rock',
    'color_snow',
    'color_sky',
    'color_sky_top',
    'color_sky_horizon',
];
