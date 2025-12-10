import type { TerrainSettings, SkySettings, TerrainColorInputs, SkyColorInputs, ChangeCategory } from './types';
import { SLIDER_CONFIGS, COLOR_EPSILON, NUMBER_EPSILON, GENERATION_SETTINGS, COLOR_SETTINGS, SEED_MAX } from './constants';

// Seed generation
export function generateSeed(): number {
    return Math.floor(Math.random() * SEED_MAX);
}

// Convert RGB 0-1 to hex color
export function rgbToHex(r: number, g: number, b: number): string {
    const toHex = (c: number) => Math.round(c * 255).toString(16).padStart(2, '0');
    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

// Convert hex color to RGB 0-1
export function hexToRgb(hex: string): [number, number, number] {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    if (result) {
        return [
            parseInt(result[1], 16) / 255,
            parseInt(result[2], 16) / 255,
            parseInt(result[3], 16) / 255,
        ];
    }
    return [0, 0, 0];
}

// Compare two RGB color arrays with tolerance
export function colorsEqual(a: [number, number, number], b: [number, number, number]): boolean {
    return Math.abs(a[0] - b[0]) < COLOR_EPSILON &&
           Math.abs(a[1] - b[1]) < COLOR_EPSILON &&
           Math.abs(a[2] - b[2]) < COLOR_EPSILON;
}

// Compare two numbers with tolerance
export function numbersEqual(a: number, b: number): boolean {
    return Math.abs(a - b) < NUMBER_EPSILON;
}

// Transform slider position to actual value (exponential curve)
export function sliderToValue(sliderId: string, sliderValue: number): number {
    const config = SLIDER_CONFIGS[sliderId];
    if (!config) return sliderValue;  // Linear fallback for unconfigured sliders

    const normalized = (sliderValue - config.min) / (config.max - config.min);
    const curved = Math.pow(normalized, config.exponent);
    return config.min + curved * (config.max - config.min);
}

// Transform actual value to slider position (inverse of above)
export function valueToSlider(sliderId: string, actualValue: number): number {
    const config = SLIDER_CONFIGS[sliderId];
    if (!config) return actualValue;  // Linear fallback for unconfigured sliders

    // Clamp to valid range
    const clamped = Math.max(config.min, Math.min(config.max, actualValue));
    const normalized = (clamped - config.min) / (config.max - config.min);
    const uncurved = Math.pow(normalized, 1 / config.exponent);
    return config.min + uncurved * (config.max - config.min);
}

// Format value for display based on slider config
export function formatSliderValue(sliderId: string, value: number): string {
    const config = SLIDER_CONFIGS[sliderId];
    return config ? value.toFixed(config.decimals) : value.toString();
}

// DOM element helpers
export function getInput(id: string): HTMLInputElement {
    return document.getElementById(id) as HTMLInputElement;
}

export function getSelect(id: string): HTMLSelectElement {
    return document.getElementById(id) as HTMLSelectElement;
}

export function getValueSpan(id: string): HTMLSpanElement {
    return document.getElementById(id) as HTMLSpanElement;
}

// Set slider value and display directly (for elements already retrieved)
export function setSliderAndDisplay(slider: HTMLInputElement, display: HTMLSpanElement, value: number, decimals?: number) {
    slider.value = value.toString();
    display.textContent = decimals !== undefined ? value.toFixed(decimals) : value.toString();
}

// Conditionally set slider value if defined
export function setSliderValueIfDefined(value: number | undefined, slider: HTMLInputElement, display: HTMLSpanElement, decimals?: number) {
    if (value === undefined) return;
    setSliderAndDisplay(slider, display, value, decimals);
}

/**
 * Set slider value by ID with display update.
 * For non-linear sliders, converts actual value to slider position.
 * (Renamed from setInputValue for clarity)
 */
export function setSliderValue(id: string, value: number, decimals?: number) {
    const input = document.getElementById(id) as HTMLInputElement;
    const valueDisplay = document.getElementById(`${id}-value`) as HTMLSpanElement;
    if (input) {
        // Convert actual value to slider position for non-linear sliders
        const sliderPos = valueToSlider(id, value);
        input.value = sliderPos.toString();
    }
    if (valueDisplay) {
        // Display shows the actual value, not slider position
        const config = SLIDER_CONFIGS[id];
        if (config) {
            valueDisplay.textContent = formatSliderValue(id, value);
        } else {
            valueDisplay.textContent = decimals !== undefined ? value.toFixed(decimals) : value.toString();
        }
    }
}

/**
 * Set color input from RGB array.
 * (Renamed from setColorValue for clarity)
 */
export function setColorInputValue(id: string, rgb: [number, number, number]) {
    const input = document.getElementById(id) as HTMLInputElement;
    if (input) {
        input.value = rgbToHex(rgb[0], rgb[1], rgb[2]);
    }
}

// Set all terrain color inputs at once
export function setTerrainColorInputs(colors: TerrainColorInputs) {
    setColorInputValue('color-abyss', colors.color_abyss);
    setColorInputValue('color-deep-water', colors.color_deep_water);
    setColorInputValue('color-shallow-water', colors.color_shallow_water);
    setColorInputValue('color-sand', colors.color_sand);
    setColorInputValue('color-grass', colors.color_grass);
    setColorInputValue('color-rock', colors.color_rock);
    setColorInputValue('color-snow', colors.color_snow);
    setColorInputValue('color-sky', colors.color_sky);
    setColorInputValue('color-sky-top', colors.color_sky_top);
    setColorInputValue('color-sky-horizon', colors.color_sky_horizon);
}

// Set all sky color inputs at once
export function setSkyColorInputs(colors: SkyColorInputs) {
    setColorInputValue('star-color', colors.star_color);
    setColorInputValue('sun-color', colors.sun_color);
    setColorInputValue('moon-color', colors.moon_color);
}

// Set particle color input (RGBA -> RGB for input)
export function setParticleColorInput(color: [number, number, number, number]) {
    setColorInputValue('particle-color', [color[0], color[1], color[2]]);
}

// Determine what category of settings changed
export function categorizeChanges(current: TerrainSettings, previous: TerrainSettings): ChangeCategory {
    let generationChanged = false;
    let colorsChanged = false;

    // Check generation settings
    for (const key of GENERATION_SETTINGS) {
        const currVal = current[key] as number;
        const prevVal = previous[key] as number;
        if (!numbersEqual(currVal, prevVal)) {
            generationChanged = true;
        }
    }

    // Check color settings
    for (const key of COLOR_SETTINGS) {
        const currVal = current[key] as [number, number, number];
        const prevVal = previous[key] as [number, number, number];
        if (!colorsEqual(currVal, prevVal)) {
            colorsChanged = true;
        }
    }

    if (generationChanged) {
        return 'generation';
    } else if (colorsChanged) {
        return 'colors_only';
    }
    return 'none';
}

// Helper to set up slider event listener with appropriate formatting
export function setupSliderListener(sliderId: string, decimals?: number) {
    const slider = document.getElementById(sliderId) as HTMLInputElement;
    const valueDisplay = document.getElementById(`${sliderId}-value`) as HTMLSpanElement;
    if (!slider || !valueDisplay) return;

    slider.addEventListener('input', () => {
        const config = SLIDER_CONFIGS[sliderId];
        if (config) {
            // Non-linear slider: transform value and use config formatting
            const actualValue = sliderToValue(sliderId, parseFloat(slider.value));
            valueDisplay.textContent = formatSliderValue(sliderId, actualValue);
        } else if (decimals !== undefined) {
            // Linear slider with decimal formatting
            valueDisplay.textContent = parseFloat(slider.value).toFixed(decimals);
        } else {
            // Linear slider with integer formatting (raw value)
            valueDisplay.textContent = slider.value;
        }
    });
}
