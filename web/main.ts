import init, { update_terrain_settings, get_terrain_settings, regenerate_terrain, update_sky_settings, get_sky_settings, update_particle_settings, get_particle_settings, get_default_terrain_settings, get_default_sky_settings, get_default_particle_settings, get_preset_list, get_preset, get_default_preset_id } from '../pkg/procedural_terrain_simulator.js';

// Import types
import type { TerrainSettings, SkySettings, ParticleSettings, PresetInfo, FullPreset } from './types';

// Import constants
import {
    COLOR_THEMES,
    SKY_PRESETS,
    WEATHER_PRESETS,
    SLIDER_SETUP,
    SNOW_WEATHER_TYPES,
} from './constants';

// Import utilities
import {
    generateSeed,
    rgbToHex,
    hexToRgb,
    colorsEqual,
    sliderToValue,
    valueToSlider,
    formatSliderValue,
    getInput,
    getSelect,
    setSliderAndDisplay,
    setSliderValueIfDefined,
    setSliderValue,
    setColorInputValue,
    setTerrainColorInputs,
    setSkyColorInputs,
    setParticleColorInput,
    categorizeChanges,
    setupSliderListener,
} from './utils';

// Module-level variables for defaults fetched from WASM
let DEFAULT_TERRAIN_SETTINGS: TerrainSettings;
let DEFAULT_SKY_SETTINGS: SkySettings;
let DEFAULT_PARTICLE_SETTINGS: ParticleSettings;
let currentParticleSize: number | undefined;

// Module-level variables for presets
let AVAILABLE_PRESETS: PresetInfo[] = [];
let currentPresetId: string | null = null;
let savedCustomSettings: {
    terrain: TerrainSettings;
    sky: SkySettings;
    particles: ParticleSettings;
} | null = null;

// Track last applied settings to detect changes (module scope for access from selectPreset)
let lastAppliedSettings: TerrainSettings;
let lastAppliedSkySettings: SkySettings;
let lastAppliedParticleSettings: ParticleSettings;

// Detect which color theme matches the current terrain settings
function detectTheme(settings: TerrainSettings): string | null {
    for (const [themeName, theme] of Object.entries(COLOR_THEMES)) {
        if (colorsEqual(settings.color_abyss, theme.abyss) &&
            colorsEqual(settings.color_deep_water, theme.deep_water) &&
            colorsEqual(settings.color_shallow_water, theme.shallow_water) &&
            colorsEqual(settings.color_sand, theme.sand) &&
            colorsEqual(settings.color_grass, theme.grass) &&
            colorsEqual(settings.color_rock, theme.rock) &&
            colorsEqual(settings.color_snow, theme.snow)) {
            return themeName;
        }
    }
    return null; // Custom colors don't match any theme
}

// Detect which sky preset matches the current sky settings
function detectSkyPreset(settings: SkySettings): string | null {
    for (const [presetName, preset] of Object.entries(SKY_PRESETS)) {
        // Check if all defined preset values match the settings
        if (preset.star_count !== undefined && preset.star_count !== settings.star_count) continue;
        if (preset.sun_count !== undefined && preset.sun_count !== settings.sun_count) continue;
        if (preset.moon_count !== undefined && preset.moon_count !== settings.moon_count) continue;
        return presetName;
    }
    return null; // Custom settings don't match any preset
}

function buildParticleSettingsFromValues(
    densityValue: string,
    speedValue: string,
    windXValue: string,
    windZValue: string,
    colorHex: string,
    weatherPreset: string,
    options: { logWarnings?: boolean } = {},
    overrides: { particleSize?: number } = {},
): ParticleSettings {
    const warn = (message: string) => {
        if (options.logWarnings) {
            console.warn(message);
        }
    };

    let density = parseFloat(densityValue);
    if (isNaN(density) || density < 0) {
        warn('Invalid density value, defaulting to 0');
        density = 0;
    }

    let speed = parseFloat(speedValue);
    if (isNaN(speed) || speed < 0) {
        warn('Invalid speed value, using default');
        speed = DEFAULT_PARTICLE_SETTINGS.speed;
    }

    let windX = parseFloat(windXValue);
    if (isNaN(windX)) {
        windX = 0;
    }

    let windZ = parseFloat(windZValue);
    if (isNaN(windZ)) {
        windZ = 0;
    }

    const rgb = hexToRgb(colorHex);
    const isSnow = SNOW_WEATHER_TYPES.has(weatherPreset);
    const defaultSize = DEFAULT_PARTICLE_SETTINGS ? DEFAULT_PARTICLE_SETTINGS.particle_size : 0.5;
    const particleSize = overrides.particleSize ?? currentParticleSize ?? defaultSize;
    const maxParticles = DEFAULT_PARTICLE_SETTINGS ? DEFAULT_PARTICLE_SETTINGS.max_particles : 0;

    return {
        particle_type: density > 0 ? (isSnow ? 1 : 0) : 0,
        density,
        max_particles: maxParticles,
        speed,
        wind_x: windX,
        wind_z: windZ,
        particle_size: particleSize,
        particle_color: [rgb[0], rgb[1], rgb[2], 0.6],
        spawn_height: DEFAULT_PARTICLE_SETTINGS.spawn_height,
        spawn_radius: DEFAULT_PARTICLE_SETTINGS.spawn_radius,
    };
}

// Populate the preset buttons from WASM data
function populatePresetButtons() {
    const presetContainer = document.getElementById('preset-buttons') as HTMLElement;
    if (!presetContainer) return;

    // Clear existing buttons
    presetContainer.innerHTML = '';

    // Add buttons from Rust
    for (const preset of AVAILABLE_PRESETS) {
        const button = document.createElement('span');
        button.className = 'preset-item';
        button.dataset.presetId = preset.id;
        button.textContent = preset.name;
        if (preset.id === currentPresetId) {
            button.classList.add('active');
        }
        button.addEventListener('click', () => selectPreset(preset.id));
        presetContainer.appendChild(button);
    }

    // Add "Custom" button for when user modifies settings
    const customButton = document.createElement('span');
    customButton.className = 'preset-item';
    customButton.dataset.presetId = 'custom';
    customButton.textContent = 'Custom';
    if (currentPresetId === 'custom') {
        customButton.classList.add('active');
    }
    customButton.addEventListener('click', () => selectPreset('custom'));
    presetContainer.appendChild(customButton);
}

/**
 * Collect current terrain settings from UI.
 * Applies non-linear transformation for configured sliders.
 * @param seed - Optional seed value. If not provided, generates a new seed.
 */
function collectTerrainSettings(seed?: number): TerrainSettings {
    const base = lastAppliedSettings ?? DEFAULT_TERRAIN_SETTINGS;
    return {
        terrain_scale: sliderToValue('terrain-scale', parseFloat(getInput('terrain-scale').value)),
        height_scale: parseFloat(getInput('height-scale').value),
        octaves: parseInt(getInput('octaves').value),
        warp_strength: sliderToValue('warp-strength', parseFloat(getInput('warp-strength').value)),
        height_variance: parseFloat(getInput('height-variance').value),
        roughness: sliderToValue('roughness', parseFloat(getInput('roughness').value)),
        pattern_type: parseInt(getSelect('pattern-type').value),
        seed: seed ?? generateSeed(),
        ambient: base.ambient,
        fog_start: base.fog_start,
        fog_distance: base.fog_distance,
        color_abyss: hexToRgb(getInput('color-abyss').value),
        color_deep_water: hexToRgb(getInput('color-deep-water').value),
        color_shallow_water: hexToRgb(getInput('color-shallow-water').value),
        color_sand: hexToRgb(getInput('color-sand').value),
        color_grass: hexToRgb(getInput('color-grass').value),
        color_rock: hexToRgb(getInput('color-rock').value),
        color_snow: hexToRgb(getInput('color-snow').value),
        color_sky: hexToRgb(getInput('color-sky').value),
        color_sky_top: hexToRgb(getInput('color-sky-top').value),
        color_sky_horizon: hexToRgb(getInput('color-sky-horizon').value),
    };
}

/**
 * Collect current sky settings from UI.
 * @param seed - Optional seed value. If not provided, generates a new seed.
 */
function collectSkySettings(seed?: number): SkySettings {
    const base = lastAppliedSkySettings ?? DEFAULT_SKY_SETTINGS;
    return {
        star_count: parseInt(getInput('star-count').value),
        star_size_min: base.star_size_min,
        star_size_max: base.star_size_max,
        star_color: hexToRgb(getInput('star-color').value),
        star_twinkle_speed: base.star_twinkle_speed,
        star_parallax: base.star_parallax,
        sun_count: parseInt(getInput('sun-count').value),
        sun_size: base.sun_size,
        sun_color: hexToRgb(getInput('sun-color').value),
        sun_parallax: base.sun_parallax,
        moon_count: parseInt(getInput('moon-count').value),
        moon_size: base.moon_size,
        moon_color: hexToRgb(getInput('moon-color').value),
        moon_parallax: base.moon_parallax,
        seed: seed ?? generateSeed(),
    };
}

/**
 * Collect current particle settings from UI.
 * @param options - Optional settings for logging warnings.
 */
function collectParticleSettings(options?: { logWarnings?: boolean }): ParticleSettings {
    const baseSize = currentParticleSize ?? (DEFAULT_PARTICLE_SETTINGS ? DEFAULT_PARTICLE_SETTINGS.particle_size : 0.5);
    return buildParticleSettingsFromValues(
        getInput('particle-density').value,
        getInput('particle-speed').value,
        getInput('wind-x').value,
        getInput('wind-z').value,
        getInput('particle-color').value,
        getSelect('weather-preset').value,
        options,
        { particleSize: baseSize }
    );
}

// Helper to sync lastApplied variables from current UI state
// Called after preset selection and initialization to ensure Apply button comparisons work correctly
function syncLastAppliedFromUI(terrainSeed: number, skySeed: number) {
    lastAppliedSettings = collectTerrainSettings(terrainSeed);
    lastAppliedSkySettings = collectSkySettings(skySeed);
    lastAppliedParticleSettings = collectParticleSettings();
    currentParticleSize = lastAppliedParticleSettings.particle_size;
}

// Select a preset by ID
function selectPreset(presetId: string) {
    if (presetId === 'custom') {
        // Restore saved custom settings if available
        if (savedCustomSettings) {
            // Apply saved settings to UI
            applyPresetToUI({
                name: 'Custom',
                terrain: savedCustomSettings.terrain,
                sky: savedCustomSettings.sky,
                particles: savedCustomSettings.particles,
            } as FullPreset);

            // Update WASM with saved settings
            update_terrain_settings(savedCustomSettings.terrain);
            regenerate_terrain();
            update_sky_settings(savedCustomSettings.sky);
            update_particle_settings(savedCustomSettings.particles);

            // Sync lastApplied to avoid false change detection on next Apply
            syncLastAppliedFromUI(savedCustomSettings.terrain.seed, savedCustomSettings.sky.seed);
        }
        setActivePreset('custom');
        return;
    }

    // Save current settings before switching away from custom
    if (currentPresetId === 'custom') {
        savedCustomSettings = {
            terrain: collectTerrainSettings(),
            sky: collectSkySettings(),
            particles: collectParticleSettings(),
        };
    }

    try {
        // Fetch the full preset from WASM
        const preset = get_preset(presetId) as FullPreset;

        // Apply preset to UI
        applyPresetToUI(preset);

        // Apply terrain settings with random seeds
        const terrainSettings = {
            ...preset.terrain,
            seed: generateSeed(),
        };
        const skySettings = { ...preset.sky, seed: generateSeed() };

        update_terrain_settings(terrainSettings);
        regenerate_terrain();
        update_sky_settings(skySettings);
        update_particle_settings(preset.particles);

        // Update active preset
        setActivePreset(presetId);

        // Keep last applied settings in sync with the preset values (including seeds and particle size)
        lastAppliedSettings = { ...terrainSettings };
        lastAppliedSkySettings = { ...skySettings };
        lastAppliedParticleSettings = { ...preset.particles };
        currentParticleSize = preset.particles.particle_size;
    } catch (e) {
        console.error('Failed to apply preset:', e);
    }
}

// Update the active preset button
function setActivePreset(presetId: string) {
    currentPresetId = presetId;
    const presetContainer = document.getElementById('preset-buttons') as HTMLElement;
    if (!presetContainer) return;

    // Remove active class from all buttons
    presetContainer.querySelectorAll('.preset-item').forEach(item => {
        item.classList.remove('active');
    });

    // Add active class to the selected button
    const activeButton = presetContainer.querySelector(`[data-preset-id="${presetId}"]`);
    if (activeButton) {
        activeButton.classList.add('active');
    }
}

// Apply a preset's values to all UI controls
function applyPresetToUI(preset: FullPreset) {
    // Update terrain generation controls
    const patternTypeSelect = document.getElementById('pattern-type') as HTMLSelectElement;
    if (patternTypeSelect) patternTypeSelect.value = preset.terrain.pattern_type.toString();

    setSliderValue('terrain-scale', preset.terrain.terrain_scale, 3);
    setSliderValue('height-scale', preset.terrain.height_scale);
    setSliderValue('height-variance', preset.terrain.height_variance, 2);
    setSliderValue('octaves', preset.terrain.octaves);
    setSliderValue('warp-strength', preset.terrain.warp_strength);
    setSliderValue('roughness', preset.terrain.roughness, 2);

    // Update color controls
    setTerrainColorInputs(preset.terrain);

    // Update sky controls
    setSliderValue('star-count', preset.sky.star_count);
    setSliderValue('sun-count', preset.sky.sun_count);
    setSliderValue('moon-count', preset.sky.moon_count);
    setSkyColorInputs(preset.sky);

    // Update particle controls
    setSliderValue('particle-density', preset.particles.density, 1);
    setSliderValue('particle-speed', preset.particles.speed);
    setSliderValue('wind-x', preset.particles.wind_x);
    setSliderValue('wind-z', preset.particles.wind_z);
    setParticleColorInput(preset.particles.particle_color);
    currentParticleSize = preset.particles.particle_size;

    // Detect and set color theme dropdown based on preset colors
    const detectedTheme = detectTheme(preset.terrain);
    const colorThemeSelect = document.getElementById('color-theme') as HTMLSelectElement;
    if (colorThemeSelect && detectedTheme) {
        colorThemeSelect.value = detectedTheme;
    }

    // Reset existing partial presets to "none" since we're applying a full preset
    const skyPresetSelect = document.getElementById('sky-preset') as HTMLSelectElement;
    const weatherPresetSelect = document.getElementById('weather-preset') as HTMLSelectElement;
    if (skyPresetSelect) skyPresetSelect.value = 'none';
    if (weatherPresetSelect) weatherPresetSelect.value = 'none';
}

// Populate all HTML inputs with default values from WASM
function populateHTMLDefaults() {
    // Terrain generation settings
    const patternTypeSelect = document.getElementById('pattern-type') as HTMLSelectElement;
    if (patternTypeSelect) {
        patternTypeSelect.value = DEFAULT_TERRAIN_SETTINGS.pattern_type.toString();
    }
    setSliderValue('terrain-scale', DEFAULT_TERRAIN_SETTINGS.terrain_scale, 3);
    setSliderValue('height-scale', DEFAULT_TERRAIN_SETTINGS.height_scale);
    setSliderValue('height-variance', DEFAULT_TERRAIN_SETTINGS.height_variance, 2);
    setSliderValue('octaves', DEFAULT_TERRAIN_SETTINGS.octaves);
    setSliderValue('warp-strength', DEFAULT_TERRAIN_SETTINGS.warp_strength);
    setSliderValue('roughness', DEFAULT_TERRAIN_SETTINGS.roughness, 2);

    // Terrain colors
    setTerrainColorInputs(DEFAULT_TERRAIN_SETTINGS);

    // Sky settings
    setSliderValue('star-count', DEFAULT_SKY_SETTINGS.star_count);
    setSliderValue('sun-count', DEFAULT_SKY_SETTINGS.sun_count);
    setSliderValue('moon-count', DEFAULT_SKY_SETTINGS.moon_count);
    setSkyColorInputs(DEFAULT_SKY_SETTINGS);

    // Particle/weather settings
    setSliderValue('particle-density', DEFAULT_PARTICLE_SETTINGS.density, 1);
    setSliderValue('particle-speed', DEFAULT_PARTICLE_SETTINGS.speed);
    setSliderValue('wind-x', DEFAULT_PARTICLE_SETTINGS.wind_x);
    setSliderValue('wind-z', DEFAULT_PARTICLE_SETTINGS.wind_z);
    setParticleColorInput(DEFAULT_PARTICLE_SETTINGS.particle_color);

    // Detect and set theme dropdown
    const detectedTheme = detectTheme(DEFAULT_TERRAIN_SETTINGS);
    if (detectedTheme) {
        const colorThemeSelect = document.getElementById('color-theme') as HTMLSelectElement;
        if (colorThemeSelect) {
            colorThemeSelect.value = detectedTheme;
        }
    }
}

async function main() {
    const canvas = document.getElementById('canvas') as HTMLCanvasElement;
    const errorEl = document.getElementById('error') as HTMLElement;

    // Check WebGPU support
    if (!navigator.gpu) {
        errorEl.textContent = 'WebGPU is not supported in this browser. Please use Chrome 113+, Edge 113+, or Firefox with WebGPU enabled.';
        errorEl.style.display = 'block';
        canvas.style.display = 'none';
        return;
    }

    // Set canvas size
    const dpr = window.devicePixelRatio || 1;
    const logicalWidth = window.innerWidth;
    const logicalHeight = window.innerHeight;
    canvas.style.width = `${logicalWidth}px`;
    canvas.style.height = `${logicalHeight}px`;
    canvas.width = Math.round(logicalWidth * dpr);
    canvas.height = Math.round(logicalHeight * dpr);

    try {
        // Initialize WASM module
        await init();

        // Fetch default settings from Rust (single source of truth)
        DEFAULT_TERRAIN_SETTINGS = get_default_terrain_settings() as TerrainSettings;
        DEFAULT_SKY_SETTINGS = get_default_sky_settings() as SkySettings;
        DEFAULT_PARTICLE_SETTINGS = get_default_particle_settings() as ParticleSettings;
        currentParticleSize = DEFAULT_PARTICLE_SETTINGS.particle_size;

        // Fetch available presets from Rust
        AVAILABLE_PRESETS = get_preset_list() as PresetInfo[];
        currentPresetId = get_default_preset_id();

        // Populate preset buttons
        populatePresetButtons();

        // Populate HTML inputs with defaults
        populateHTMLDefaults();

        // Initialize lastApplied from UI values to avoid false positives on first Apply
        syncLastAppliedFromUI(DEFAULT_TERRAIN_SETTINGS.seed, DEFAULT_SKY_SETTINGS.seed);

        // Setup settings panel after WASM is initialized
        setupSettingsPanel();
    } catch (e) {
        errorEl.textContent = `Failed to initialize: ${e}`;
        errorEl.style.display = 'block';
        console.error(e);
    }
}

function setupSettingsPanel() {
    const settingsPanel = document.getElementById('settings-panel') as HTMLElement;
    const settingsToggle = document.getElementById('settings-toggle') as HTMLButtonElement;
    const applyButton = document.getElementById('apply-settings') as HTMLButtonElement;
    const resetButton = document.getElementById('reset-settings') as HTMLButtonElement;

    // Generation sliders
    const patternTypeSelect = document.getElementById('pattern-type') as HTMLSelectElement;
    const terrainScaleSlider = document.getElementById('terrain-scale') as HTMLInputElement;
    const heightScaleSlider = document.getElementById('height-scale') as HTMLInputElement;
    const heightVarianceSlider = document.getElementById('height-variance') as HTMLInputElement;
    const octavesSlider = document.getElementById('octaves') as HTMLInputElement;
    const warpStrengthSlider = document.getElementById('warp-strength') as HTMLInputElement;
    const roughnessSlider = document.getElementById('roughness') as HTMLInputElement;

    // Value displays
    const terrainScaleValue = document.getElementById('terrain-scale-value') as HTMLSpanElement;
    const heightScaleValue = document.getElementById('height-scale-value') as HTMLSpanElement;
    const heightVarianceValue = document.getElementById('height-variance-value') as HTMLSpanElement;
    const octavesValue = document.getElementById('octaves-value') as HTMLSpanElement;
    const warpStrengthValue = document.getElementById('warp-strength-value') as HTMLSpanElement;
    const roughnessValue = document.getElementById('roughness-value') as HTMLSpanElement;

    // Color controls
    const colorThemeSelect = document.getElementById('color-theme') as HTMLSelectElement;

    // Sky object controls
    const skyPresetSelect = document.getElementById('sky-preset') as HTMLSelectElement;
    const starCountSlider = document.getElementById('star-count') as HTMLInputElement;
    const sunCountSlider = document.getElementById('sun-count') as HTMLInputElement;
    const moonCountSlider = document.getElementById('moon-count') as HTMLInputElement;
    const starCountValue = document.getElementById('star-count-value') as HTMLSpanElement;
    const sunCountValue = document.getElementById('sun-count-value') as HTMLSpanElement;
    const moonCountValue = document.getElementById('moon-count-value') as HTMLSpanElement;
    const starColor = document.getElementById('star-color') as HTMLInputElement;
    const sunColor = document.getElementById('sun-color') as HTMLInputElement;
    const moonColor = document.getElementById('moon-color') as HTMLInputElement;

    // Weather/particle controls
    const weatherPresetSelect = document.getElementById('weather-preset') as HTMLSelectElement;
    const particleDensitySlider = document.getElementById('particle-density') as HTMLInputElement;
    const particleSpeedSlider = document.getElementById('particle-speed') as HTMLInputElement;
    const windXSlider = document.getElementById('wind-x') as HTMLInputElement;
    const windZSlider = document.getElementById('wind-z') as HTMLInputElement;
    const particleDensityValue = document.getElementById('particle-density-value') as HTMLSpanElement;
    const particleSpeedValue = document.getElementById('particle-speed-value') as HTMLSpanElement;
    const windXValue = document.getElementById('wind-x-value') as HTMLSpanElement;
    const windZValue = document.getElementById('wind-z-value') as HTMLSpanElement;
    const particleColor = document.getElementById('particle-color') as HTMLInputElement;

    const updateToggleLabel = () => {
        settingsToggle.textContent = settingsPanel.classList.contains('collapsed') ? '+' : 'A-';
    };

    // Toggle settings panel
    settingsToggle.addEventListener('click', () => {
        settingsPanel.classList.toggle('collapsed');
        updateToggleLabel();
    });

    // Keyboard shortcuts - Tab cycles: Expanded → Collapsed → Hidden → Expanded
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Tab') {
            e.preventDefault();

            if (settingsPanel.classList.contains('hidden')) {
                // Hidden → Expanded
                settingsPanel.classList.remove('hidden');
            } else if (settingsPanel.classList.contains('collapsed')) {
                // Collapsed → Hidden
                settingsPanel.classList.remove('collapsed');
                settingsPanel.classList.add('hidden');
            } else {
                // Expanded → Collapsed
                settingsPanel.classList.add('collapsed');
            }
            updateToggleLabel();
        }
        if (e.key === 'r' || e.key === 'R') {
            try {
                // Get current settings and apply with new random seed
                const currentSettings = get_terrain_settings() as TerrainSettings;
                currentSettings.seed = generateSeed();
                update_terrain_settings(currentSettings);
                regenerate_terrain();
                // Update last applied settings from UI to stay in sync
                lastAppliedSettings = collectTerrainSettings(currentSettings.seed);
            } catch (err) {
                console.error('Failed to regenerate terrain:', err);
            }
        }
    });

    // Setup all slider listeners using configuration
    SLIDER_SETUP.forEach(({ id, decimals }) => setupSliderListener(id, decimals));

    // Apply color theme
    colorThemeSelect.addEventListener('change', () => {
        const theme = COLOR_THEMES[colorThemeSelect.value];
        if (theme) {
            setTerrainColorInputs({
                color_abyss: theme.abyss,
                color_deep_water: theme.deep_water,
                color_shallow_water: theme.shallow_water,
                color_sand: theme.sand,
                color_grass: theme.grass,
                color_rock: theme.rock,
                color_snow: theme.snow,
                color_sky: theme.sky,
                color_sky_top: theme.sky_top,
                color_sky_horizon: theme.sky_horizon,
            });
            setSkyColorInputs({
                star_color: theme.star_color,
                sun_color: theme.sun_color,
                moon_color: theme.moon_color,
            });
        }
    });

    // Apply sky preset
    skyPresetSelect.addEventListener('change', () => {
        const preset = SKY_PRESETS[skyPresetSelect.value];
        if (preset) {
            setSliderValueIfDefined(preset.star_count, starCountSlider, starCountValue);
            setSliderValueIfDefined(preset.sun_count, sunCountSlider, sunCountValue);
            setSliderValueIfDefined(preset.moon_count, moonCountSlider, moonCountValue);
            if (preset.star_color) {
                starColor.value = rgbToHex(...preset.star_color);
            }
            if (preset.sun_color) {
                sunColor.value = rgbToHex(...preset.sun_color);
            }
            if (preset.moon_color) {
                moonColor.value = rgbToHex(...preset.moon_color);
            }
        }
    });

    // Apply weather preset
    weatherPresetSelect.addEventListener('change', () => {
        const preset = WEATHER_PRESETS[weatherPresetSelect.value];
        if (preset) {
            setSliderValueIfDefined(preset.density, particleDensitySlider, particleDensityValue, 1);
            setSliderValueIfDefined(preset.speed, particleSpeedSlider, particleSpeedValue);
            setSliderValueIfDefined(preset.wind_x, windXSlider, windXValue);
            setSliderValueIfDefined(preset.wind_z, windZSlider, windZValue);
            if (preset.particle_color) {
                particleColor.value = rgbToHex(preset.particle_color[0], preset.particle_color[1], preset.particle_color[2]);
            }
            if (preset.particle_size !== undefined) {
                currentParticleSize = preset.particle_size;
            } else if (DEFAULT_PARTICLE_SETTINGS) {
                currentParticleSize = DEFAULT_PARTICLE_SETTINGS.particle_size;
            }
        }
    });

    // Apply settings with smart change detection
    applyButton.addEventListener('click', () => {
        try {
            // Get current seed from WASM
            const wasmSettings = get_terrain_settings() as TerrainSettings;
            const currentSeed = wasmSettings.seed;

            // Collect current UI values (preserving current seed)
            const currentUISettings = collectTerrainSettings(currentSeed);

            // Categorize what changed (compare against last applied settings)
            const changeCategory = categorizeChanges(currentUISettings, lastAppliedSettings);

            switch (changeCategory) {
                case 'none':
                    // No changes detected, nothing to do
                    break;

                case 'colors_only':
                    // Keep the same seed, just update settings (no regeneration)
                    update_terrain_settings(currentUISettings);
                    lastAppliedSettings = { ...currentUISettings };
                    break;

                case 'generation':
                    // Generation settings changed: new seed + regenerate
                    currentUISettings.seed = generateSeed();
                    update_terrain_settings(currentUISettings);
                    regenerate_terrain();
                    lastAppliedSettings = { ...currentUISettings };
                    break;
            }

            // Always update sky settings (fast update, no regeneration concept)
            const currentSkySeed = lastAppliedSkySettings.seed;
            const currentSkyUISettings = collectSkySettings(currentSkySeed);

            // Check if sky settings changed
            const skyChanged =
                currentSkyUISettings.star_count !== lastAppliedSkySettings.star_count ||
                currentSkyUISettings.sun_count !== lastAppliedSkySettings.sun_count ||
                currentSkyUISettings.moon_count !== lastAppliedSkySettings.moon_count ||
                !colorsEqual(currentSkyUISettings.star_color, lastAppliedSkySettings.star_color) ||
                !colorsEqual(currentSkyUISettings.sun_color, lastAppliedSkySettings.sun_color) ||
                !colorsEqual(currentSkyUISettings.moon_color, lastAppliedSkySettings.moon_color);

            if (skyChanged) {
                // Use new seed if object counts changed
                if (currentSkyUISettings.star_count !== lastAppliedSkySettings.star_count ||
                    currentSkyUISettings.sun_count !== lastAppliedSkySettings.sun_count ||
                    currentSkyUISettings.moon_count !== lastAppliedSkySettings.moon_count) {
                    currentSkyUISettings.seed = generateSeed();
                }
                update_sky_settings(currentSkyUISettings);
                lastAppliedSkySettings = { ...currentSkyUISettings };
            }

            // Update particle settings (fast update, always apply)
            const currentParticleSettings = collectParticleSettings({ logWarnings: true });
            update_particle_settings(currentParticleSettings);
            lastAppliedParticleSettings = { ...currentParticleSettings };
            currentParticleSize = currentParticleSettings.particle_size;

            // Mark as custom if any settings were changed
            if (changeCategory !== 'none' || skyChanged) {
                if (currentPresetId !== 'custom') {
                    setActivePreset('custom');
                }
            }
        } catch (e) {
            console.error('Failed to apply settings:', e);
        }
    });

    // Regenerate with current settings + new seed (like pressing R key)
    resetButton.addEventListener('click', () => {
        try {
            // Get current settings from WASM
            const currentSettings = get_terrain_settings() as TerrainSettings;

            // Generate new seed
            currentSettings.seed = generateSeed();

            // Update and regenerate
            update_terrain_settings(currentSettings);
            regenerate_terrain();

            // Update last applied settings from UI to reflect the regenerated state
            lastAppliedSettings = { ...currentSettings };
        } catch (err) {
            console.error('Failed to regenerate terrain:', err);
        }
    });

    // Load current settings from WASM
    try {
        const currentSettings = get_terrain_settings() as TerrainSettings;
        if (currentSettings) {
            // Generation settings - use inverse transform to position sliders correctly
            patternTypeSelect.value = (currentSettings.pattern_type || 0).toString();
            terrainScaleSlider.value = valueToSlider('terrain-scale', currentSettings.terrain_scale).toString();
            heightScaleSlider.value = currentSettings.height_scale.toString();
            heightVarianceSlider.value = (currentSettings.height_variance || 0.5).toString();
            octavesSlider.value = currentSettings.octaves.toString();
            warpStrengthSlider.value = valueToSlider('warp-strength', currentSettings.warp_strength).toString();
            roughnessSlider.value = valueToSlider('roughness', currentSettings.roughness || 0.5).toString();

            // Update displays - show actual values, not slider positions
            terrainScaleValue.textContent = formatSliderValue('terrain-scale', currentSettings.terrain_scale);
            heightScaleValue.textContent = currentSettings.height_scale.toString();
            heightVarianceValue.textContent = (currentSettings.height_variance || 0.5).toFixed(2);
            octavesValue.textContent = currentSettings.octaves.toString();
            warpStrengthValue.textContent = formatSliderValue('warp-strength', currentSettings.warp_strength);
            roughnessValue.textContent = formatSliderValue('roughness', currentSettings.roughness || 0.5);

            setTerrainColorInputs(currentSettings);

            // Detect and set the theme dropdown to match loaded colors
            const detectedTheme = detectTheme(currentSettings);
            if (detectedTheme) {
                colorThemeSelect.value = detectedTheme;
            }

            // Initialize lastAppliedSettings from UI values (not raw WASM values)
            // This ensures the comparison matches what the UI actually shows
            lastAppliedSettings = collectTerrainSettings(currentSettings.seed);
        }

        // Load sky settings
        const currentSkySettings = get_sky_settings() as SkySettings;
        if (currentSkySettings) {
            setSliderAndDisplay(starCountSlider, starCountValue, currentSkySettings.star_count);
            setSliderAndDisplay(sunCountSlider, sunCountValue, currentSkySettings.sun_count);
            setSliderAndDisplay(moonCountSlider, moonCountValue, currentSkySettings.moon_count);

            setSkyColorInputs(currentSkySettings);

            // Detect and set the sky preset dropdown to match loaded settings
            const detectedSkyPreset = detectSkyPreset(currentSkySettings);
            if (detectedSkyPreset) {
                skyPresetSelect.value = detectedSkyPreset;
            }

            lastAppliedSkySettings = collectSkySettings(currentSkySettings.seed);
        }

        // Load particle settings
        const currentParticleSettings = get_particle_settings() as ParticleSettings;
        if (currentParticleSettings) {
            currentParticleSize = currentParticleSettings.particle_size;
            setSliderAndDisplay(particleDensitySlider, particleDensityValue, currentParticleSettings.density, 1);
            setSliderAndDisplay(particleSpeedSlider, particleSpeedValue, currentParticleSettings.speed);
            setSliderAndDisplay(windXSlider, windXValue, currentParticleSettings.wind_x);
            setSliderAndDisplay(windZSlider, windZValue, currentParticleSettings.wind_z);

            if (currentParticleSettings.particle_color) {
                particleColor.value = rgbToHex(
                    currentParticleSettings.particle_color[0],
                    currentParticleSettings.particle_color[1],
                    currentParticleSettings.particle_color[2]
                );
            }

            lastAppliedParticleSettings = { ...currentParticleSettings };
        }
    } catch {
        // Using default settings
    }
}

main();
