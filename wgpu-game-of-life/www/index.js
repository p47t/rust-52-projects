import init, {
    start,
    step,
    render,
    reset_random,
    clear,
    toggle_cell,
    set_cell,
    get_generation,
    get_population,
    get_grid_width,
    get_grid_height,
    resize,
} from '../pkg/wgpu_game_of_life.js';

const GRID_WIDTH = 128;
const GRID_HEIGHT = 128;

// DOM elements
const canvas = document.getElementById('canvas');
const playBtn = document.getElementById('playBtn');
const stepBtn = document.getElementById('stepBtn');
const randomBtn = document.getElementById('randomBtn');
const clearBtn = document.getElementById('clearBtn');
const speedSlider = document.getElementById('speedSlider');
const speedValue = document.getElementById('speedValue');
const genCounter = document.getElementById('genCounter');
const popCounter = document.getElementById('popCounter');

let playing = false;
let stepsPerSecond = 10;
let frameAccumulator = 0;
let lastFrameTime = 0;
let animFrameId = null;
let drawing = false;
let drawValue = true; // true = set alive, false = set dead

async function run() {
    try {
        await init();
        await start('canvas', GRID_WIDTH, GRID_HEIGHT);
        console.log('WebGPU Game of Life initialized');
        updateStats();
        setupEvents();
    } catch (err) {
        console.error('Failed to initialize:', err);
        document.querySelector('.canvas-wrapper').innerHTML =
            `<div style="padding: 2rem; color: #ff6060; text-align: center;">
                <p><strong>Failed to initialize WebGPU</strong></p>
                <p style="margin-top: 0.5rem; font-size: 0.85rem;">${err.message || err}</p>
                <p style="margin-top: 0.5rem; font-size: 0.75rem; color: #8888a0;">
                    Requires a WebGPU-capable browser (Chrome 113+, Edge 113+, Firefox 141+)
                </p>
            </div>`;
    }
}

function setupEvents() {
    playBtn.addEventListener('click', togglePlay);
    stepBtn.addEventListener('click', doStep);
    randomBtn.addEventListener('click', doRandom);
    clearBtn.addEventListener('click', doClear);

    speedSlider.addEventListener('input', () => {
        stepsPerSecond = parseInt(speedSlider.value, 10);
        speedValue.textContent = stepsPerSecond;
    });

    // Canvas mouse interaction
    canvas.addEventListener('mousedown', onCanvasMouseDown);
    canvas.addEventListener('mousemove', onCanvasMouseMove);
    window.addEventListener('mouseup', onCanvasMouseUp);

    // Touch support
    canvas.addEventListener('touchstart', onCanvasTouchStart, { passive: false });
    canvas.addEventListener('touchmove', onCanvasTouchMove, { passive: false });
    canvas.addEventListener('touchend', onCanvasTouchEnd);

    // Keyboard shortcuts
    document.addEventListener('keydown', onKeyDown);
}

function togglePlay() {
    playing = !playing;
    playBtn.textContent = playing ? 'Pause' : 'Play';
    playBtn.classList.toggle('playing', playing);

    if (playing) {
        lastFrameTime = performance.now();
        frameAccumulator = 0;
        animFrameId = requestAnimationFrame(gameLoop);
    } else if (animFrameId) {
        cancelAnimationFrame(animFrameId);
        animFrameId = null;
    }
}

function gameLoop(now) {
    if (!playing) return;

    const dt = (now - lastFrameTime) / 1000; // seconds
    lastFrameTime = now;
    frameAccumulator += dt * stepsPerSecond;

    // Cap to avoid spiral of death if tab was hidden
    const maxSteps = Math.min(Math.floor(frameAccumulator), 10);
    for (let i = 0; i < maxSteps; i++) {
        step();
    }
    frameAccumulator -= maxSteps;

    if (maxSteps > 0) {
        updateStats();
    }

    animFrameId = requestAnimationFrame(gameLoop);
}

function doStep() {
    if (playing) return;
    step();
    updateStats();
}

function doRandom() {
    const wasPlaying = playing;
    if (playing) togglePlay();
    reset_random();
    updateStats();
}

function doClear() {
    const wasPlaying = playing;
    if (playing) togglePlay();
    clear();
    updateStats();
}

function updateStats() {
    genCounter.textContent = `Gen: ${get_generation()}`;
    popCounter.textContent = `Pop: ${get_population()}`;
}

// --- Canvas interaction ---

function canvasToGrid(clientX, clientY) {
    const rect = canvas.getBoundingClientRect();
    const x = (clientX - rect.left) / rect.width;
    const y = (clientY - rect.top) / rect.height;
    const gx = Math.floor(x * GRID_WIDTH);
    const gy = Math.floor(y * GRID_HEIGHT);
    return [
        Math.max(0, Math.min(gx, GRID_WIDTH - 1)),
        Math.max(0, Math.min(gy, GRID_HEIGHT - 1)),
    ];
}

function onCanvasMouseDown(e) {
    e.preventDefault();
    drawing = true;
    const [gx, gy] = canvasToGrid(e.clientX, e.clientY);
    // First click toggles; subsequent drags paint the same value
    toggle_cell(gx, gy);
    // We don't know the current cell state easily, so just set alive for drag
    drawValue = true;
    updateStats();
}

function onCanvasMouseMove(e) {
    if (!drawing) return;
    e.preventDefault();
    const [gx, gy] = canvasToGrid(e.clientX, e.clientY);
    set_cell(gx, gy, drawValue);
    updateStats();
}

function onCanvasMouseUp() {
    drawing = false;
}

function onCanvasTouchStart(e) {
    e.preventDefault();
    if (e.touches.length === 1) {
        drawing = true;
        const touch = e.touches[0];
        const [gx, gy] = canvasToGrid(touch.clientX, touch.clientY);
        toggle_cell(gx, gy);
        drawValue = true;
        updateStats();
    }
}

function onCanvasTouchMove(e) {
    e.preventDefault();
    if (!drawing || e.touches.length !== 1) return;
    const touch = e.touches[0];
    const [gx, gy] = canvasToGrid(touch.clientX, touch.clientY);
    set_cell(gx, gy, drawValue);
    updateStats();
}

function onCanvasTouchEnd() {
    drawing = false;
}

function onKeyDown(e) {
    // Don't capture if user is typing in an input
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;

    switch (e.key.toLowerCase()) {
        case ' ':
            e.preventDefault();
            togglePlay();
            break;
        case 's':
            doStep();
            break;
        case 'r':
            doRandom();
            break;
        case 'c':
            doClear();
            break;
    }
}

run();
