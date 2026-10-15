/**
 * ApiFuck's native WebGL2 lifecycle adapter for React Bits Aurora.
 * Original shaders and OKLCH color helpers: Copyright (c) 2026 David Haz.
 * MIT + Commons Clause; full notice: /third-party/react-bits-LICENSE.txt
 * https://github.com/DavidHDev/react-bits/blob/43591e8b3fa327d45633b4e8239ff6742ef706ef/src/content/Backgrounds/Aurora/Aurora.jsx
 * Adaptation: no React or OGL; bounded canvas resolution, 30 fps scheduling,
 * visibility/pause controls, context recovery, and bfcache-safe cleanup.
 */

const MAX_STOPS = 5;
const MAX_LAYERS = 3;

const VERTEX = `#version 300 es
in vec2 aPosition;
void main() {
  gl_Position = vec4(aPosition, 0.0, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision highp float;
precision highp int;
uniform vec2 uResolution;
uniform float uTime;
uniform vec3 uStops[${MAX_STOPS}];
uniform int uStopCount;
uniform float uAmplitude;
uniform float uBlend;
uniform float uHeight;
uniform float uScale;
uniform float uGlow;
uniform float uBrightness;
uniform int uLayers;
uniform float uFlip;
uniform vec3 uPointer;
uniform float uReveal;
uniform float uOpacity;
uniform float uLightMode;
out vec4 outColor;

vec3 permute(vec3 x) {
  return mod(((x * 34.0) + 1.0) * x, 289.0);
}

float snoise(vec2 v) {
  const vec4 C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
  vec2 i = floor(v + dot(v, C.yy));
  vec2 x0 = v - i + dot(i, C.xx);
  vec2 i1 = (x0.x > x0.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
  vec4 x12 = x0.xyxy + C.xxzz;
  x12.xy -= i1;
  i = mod(i, 289.0);
  vec3 p = permute(permute(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
  vec3 m = max(0.5 - vec3(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), 0.0);
  m = m * m;
  m = m * m;
  vec3 x = 2.0 * fract(p * C.www) - 1.0;
  vec3 h = abs(x) - 0.5;
  vec3 ox = floor(x + 0.5);
  vec3 a0 = x - ox;
  m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
  vec3 g;
  g.x = a0.x * x0.x + h.x * x0.y;
  g.yz = a0.yz * x12.xz + h.yz * x12.yw;
  return 130.0 * dot(m, g);
}

float smootherstep(float edge0, float edge1, float x) {
  float t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
  return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

vec3 oklabToLinear(vec3 c) {
  float l = c.x + 0.3963377774 * c.y + 0.2158037573 * c.z;
  float m = c.x - 0.1055613458 * c.y - 0.0638541728 * c.z;
  float s = c.x - 0.0894841775 * c.y - 1.2914855480 * c.z;
  l = l * l * l;
  m = m * m * m;
  s = s * s * s;
  return vec3(
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
  );
}

vec3 toSrgb(vec3 c) {
  c = clamp(c, 0.0, 1.0);
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c));
}

vec3 ramp(float t) {
  float x = clamp(t, 0.0, 1.0) * float(uStopCount - 1);
  int i = int(min(floor(x), float(uStopCount - 2)));
  float f = x - float(i);
  f = f * f * (3.0 - 2.0 * f);
  vec3 lch = mix(uStops[i], uStops[i + 1], f);
  lch.x = mix(lch.x, min(lch.x, 0.7), uLightMode);
  return toSrgb(oklabToLinear(vec3(lch.x, lch.y * cos(lch.z), lch.y * sin(lch.z))));
}

float hash(vec2 p) {
  p = fract(p * vec2(123.34, 456.21));
  p += dot(p, p + 45.32);
  return fract(p.x * p.y);
}

void main() {
  vec2 frag = gl_FragCoord.xy;
  float y = 1.0 - frag.y / uResolution.y;
  y = mix(y, 1.0 - y, uFlip);
  float u = frag.x / uResolution.x;
  float X = (frag.x - uResolution.x * 0.5) / uResolution.y;
  float t = uTime;
  float reach = uPointer.z * exp(-pow((X - uPointer.x) / 0.38, 2.0));

  vec3 glow = vec3(0.0);
  float cover = 0.0;
  for (int k = 0; k < ${MAX_LAYERS}; k++) {
    if (k >= uLayers) break;
    float layer = float(k);
    float seed = layer * 13.7;
    float pace = t * (1.0 - layer * 0.18);
    float warp = snoise(vec2(X * 0.45 * uScale + pace * 0.02 + seed, pace * 0.05 + seed));
    float wave = snoise(vec2(X * 0.75 * uScale + warp * 0.35 - pace * 0.03 + seed, pace * 0.07 - seed)) * 0.72
      + snoise(vec2(X * 1.6 * uScale + warp * 0.5 + pace * 0.04 - seed, pace * 0.1 + seed * 0.5)) * 0.28;
    float hem = uHeight + layer * 0.07 + wave * 0.15 * uAmplitude;
    hem += reach * 0.08 * (layer < 0.5 ? 1.0 : 0.4);
    hem = mix(-0.25, hem, uReveal);

    float soft = (0.04 + uBlend * 0.16) * (1.0 + layer * 0.9);
    float rise = hem - y;
    float low = -soft - uGlow * 0.14;
    float high = 0.26 + soft * 0.6;
    float body = pow(smootherstep(low, high, rise), 1.5 + uGlow * 0.6);
    float lobes = 0.86 + 0.14 * snoise(vec2(X * 0.9 * uScale - pace * 0.025 + seed, y * 0.3 + pace * 0.04));
    float alpha = body * lobes * (layer < 0.5 ? 1.0 : 0.32 / layer);
    alpha = clamp(alpha * (1.0 + reach * 0.25 * smootherstep(0.9, 0.0, abs(y - hem))), 0.0, 1.0);

    vec3 tint = ramp(u + 0.05 * sin(pace * 0.08 + layer * 2.3) + layer * 0.1 + max(rise, 0.0) * 0.05);
    glow = 1.0 - (1.0 - glow) * (1.0 - tint * alpha);
    cover = 1.0 - (1.0 - cover) * (1.0 - alpha);
  }

  float fade = smootherstep(0.0, 0.7, uReveal) * uOpacity;
  float strength = clamp(uBrightness, 0.0, 2.0);
  glow *= fade * min(strength, 1.0);
  cover *= fade * min(strength, 1.0);
  glow = 1.0 - pow(1.0 - glow, vec3(max(strength, 1.0)));
  float dither = (hash(frag) - 0.5) / 255.0;

  if (uLightMode > 0.5) {
    vec3 tint = cover > 0.0001 ? glow / cover : vec3(0.0);
    cover *= smootherstep(0.0, 0.25, cover);
    float alpha = cover * 0.7;
    outColor = vec4(tint * alpha + dither, clamp(alpha + dither, 0.0, 1.0));
    return;
  }
  outColor = vec4(glow + dither, clamp(cover + dither, 0.0, 1.0));
}`;

const clamp = (value, min, max) => Math.min(max, Math.max(min, value));

const toOklch = (rgb) => {
  const [r, g, b] = rgb.map((c) =>
    c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4),
  );
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const lab = [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
  return [lab[0], Math.hypot(lab[1], lab[2]), Math.atan2(lab[2], lab[1])];
};

const unwrapHues = (stops) => {
  const known = stops.findIndex((stop) => stop[1] > 0.02);
  const anchor = known === -1 ? 0 : stops[known][2];
  let previous = anchor;
  return stops.map((stop) => {
    let hue = stop[1] > 0.02 ? stop[2] : previous;
    while (hue - previous > Math.PI) hue -= Math.PI * 2;
    while (hue - previous < -Math.PI) hue += Math.PI * 2;
    previous = hue;
    return [stop[0], stop[1], hue];
  });
};

const FRAME_MS = 1000 / 30;
const MAX_PIXELS = 1_000_000;
const PALETTES = {
  light: ["#527BFF", "#58EDDB", "#8B71FF"],
  dark: ["#6B8EFF", "#66E2D4", "#A18AFF"],
};

function compile(gl, type, source) {
  const shader = gl.createShader(type);
  if (!shader) return null;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (gl.getShaderParameter(shader, gl.COMPILE_STATUS)) return shader;
  gl.deleteShader(shader);
  return null;
}

function createResources(gl) {
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT);
  const program = gl.createProgram();
  if (!vertex || !fragment || !program) {
    if (vertex) gl.deleteShader(vertex);
    if (fragment) gl.deleteShader(fragment);
    if (program) gl.deleteProgram(program);
    return null;
  }
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.bindAttribLocation(program, 0, "aPosition");
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    gl.deleteProgram(program);
    return null;
  }
  const buffer = gl.createBuffer();
  const vao = gl.createVertexArray();
  if (!buffer || !vao) {
    if (buffer) gl.deleteBuffer(buffer);
    if (vao) gl.deleteVertexArray(vao);
    gl.deleteProgram(program);
    return null;
  }
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 3, -1, -1, 3]),
    gl.STATIC_DRAW,
  );
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  const uniforms = {};
  const count = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < count; i += 1) {
    const info = gl.getActiveUniform(program, i);
    if (info) {
      uniforms[info.name.replace("[0]", "")] = gl.getUniformLocation(
        program,
        info.name,
      );
    }
  }
  return { program, buffer, vao, uniforms };
}

/** Mount a decorative canvas; unsupported WebGL leaves the CSS gradient visible. */
export function mountHeroAurora(canvas) {
  const inert = { setPaused() {}, setTheme() {}, destroy() {} };
  if (!canvas || typeof canvas.getContext !== "function" || !canvas.dataset) {
    return inert;
  }
  canvas.dataset.state = "fallback";
  canvas.dataset.running = "false";

  let gl = null;
  let resources = null;
  let resizeObserver = null;
  let intersection = null;
  let destroyed = false;
  let suspended = false;
  let lost = false;
  let visible = false;
  let running = false;
  let paused = true;
  let theme = "light";
  let timer = null;
  let frame = null;
  let previousTime = 0;
  let lastDraw = 0;
  const hero = canvas.closest(".hero") || canvas;
  const view = { width: 1, height: 1 };
  const pointer = { x: 0, y: 0.5, inside: false };
  const state = { clock: 0, px: 0, py: 0.5, reach: 0 };
  const palette = new Float32Array(MAX_STOPS * 3);

  function updatePalette() {
    const stops = PALETTES[theme].map((hex) =>
      toOklch(
        [1, 3, 5].map(
          (offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255,
        ),
      ),
    );
    unwrapHues(stops).forEach((stop, index) => palette.set(stop, index * 3));
  }

  function stopLoop() {
    if (timer !== null) clearTimeout(timer);
    if (frame !== null) cancelAnimationFrame(frame);
    timer = null;
    frame = null;
    previousTime = 0;
    running = false;
    pointer.inside = false;
    canvas.dataset.running = "false";
  }

  function releaseResources() {
    if (!resources) return;
    try {
      gl.deleteVertexArray(resources.vao);
      gl.deleteBuffer(resources.buffer);
      gl.deleteProgram(resources.program);
    } catch {
      // Context loss already invalidates its objects.
    }
    resources = null;
  }

  function clearCanvas() {
    try {
      if (gl && !gl.isContextLost()) {
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
      }
      // Clear any previous drawing even after an initialization failure.
      canvas.width = 1;
      canvas.height = 1;
    } catch {
      // The existing CSS gradient is the fallback when canvas is unavailable.
    }
  }

  function fallback() {
    stopLoop();
    releaseResources();
    clearCanvas();
    canvas.dataset.state = "fallback";
  }

  function canRun() {
    return (
      !destroyed &&
      !suspended &&
      !lost &&
      !!resources &&
      !paused &&
      visible &&
      !document.hidden
    );
  }

  function render() {
    if (destroyed || suspended || lost || !resources) return false;
    try {
      if (gl.isContextLost()) {
        lost = true;
        stopLoop();
        canvas.dataset.state = "lost";
        return false;
      }
      const u = resources.uniforms;
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.useProgram(resources.program);
      gl.bindVertexArray(resources.vao);
      gl.uniform2f(u.uResolution, canvas.width, canvas.height);
      gl.uniform1f(u.uTime, state.clock);
      gl.uniform3fv(u.uStops, palette);
      gl.uniform1i(u.uStopCount, PALETTES[theme].length);
      gl.uniform1f(u.uAmplitude, 1.6);
      gl.uniform1f(u.uBlend, 0.22);
      gl.uniform1f(u.uHeight, 0.42);
      gl.uniform1f(u.uScale, 1.2);
      gl.uniform1f(u.uGlow, 0.5);
      gl.uniform1f(u.uBrightness, 1);
      gl.uniform1i(u.uLayers, 2);
      gl.uniform1f(u.uFlip, 0);
      gl.uniform3f(u.uPointer, state.px, state.py, state.reach);
      // A full static first frame also serves reduced-motion visitors.
      gl.uniform1f(u.uReveal, 1);
      gl.uniform1f(u.uOpacity, 0.85);
      // Both page themes use a deep-blue hero, not Aurora's white-background mode.
      gl.uniform1f(u.uLightMode, 0);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      lastDraw = performance.now();
      return true;
    } catch {
      fallback();
      return false;
    }
  }

  function queueFrame() {
    if (!canRun() || timer !== null || frame !== null) return;
    const delay = Math.max(0, FRAME_MS - (performance.now() - lastDraw));
    timer = setTimeout(() => {
      timer = null;
      if (canRun()) frame = requestAnimationFrame(animate);
    }, delay);
  }

  function animate(now) {
    frame = null;
    if (!canRun()) {
      stopLoop();
      return;
    }
    if (now - lastDraw < FRAME_MS - 0.1) {
      queueFrame();
      return;
    }
    const dt = previousTime
      ? clamp((now - previousTime) / 1000, 0, 0.1)
      : 1 / 30;
    previousTime = now;
    state.clock += dt * 2.4;
    const follow = 1 - Math.exp(-dt / 0.6);
    state.px += ((pointer.inside ? pointer.x : 0) - state.px) * follow;
    state.py += ((pointer.inside ? pointer.y : 0.5) - state.py) * follow;
    state.reach +=
      ((pointer.inside ? 0.45 : 0) - state.reach) *
      (1 - Math.exp(-dt / (pointer.inside ? 0.9 : 1.4)));
    if (render()) queueFrame();
  }

  function synchronize() {
    if (!canRun()) {
      stopLoop();
      return;
    }
    running = true;
    canvas.dataset.running = "true";
    queueFrame();
  }

  function resize(force = false) {
    if (destroyed || suspended || lost || !resources) return;
    const rect = canvas.getBoundingClientRect();
    view.width = Math.max(1, rect.width);
    view.height = Math.max(1, rect.height);
    const ratio = Math.min(
      1,
      window.devicePixelRatio || 1,
      1440 / view.width,
      Math.sqrt(MAX_PIXELS / (view.width * view.height)),
    );
    const width = Math.max(1, Math.floor(view.width * ratio));
    const height = Math.max(1, Math.floor(view.height * ratio));
    if (force || canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
      render();
    }
    synchronize();
  }

  function initialize() {
    if (destroyed || suspended) return;
    try {
      gl = canvas.getContext("webgl2", {
        alpha: true,
        premultipliedAlpha: true,
        antialias: false,
        depth: false,
        stencil: false,
        powerPreference: "low-power",
      });
      if (!gl) {
        fallback();
        return;
      }
      if (gl.isContextLost()) {
        lost = true;
        stopLoop();
        canvas.dataset.state = "lost";
        return;
      }
      lost = false;
      releaseResources();
      resources = createResources(gl);
      if (!resources) {
        fallback();
        return;
      }
      canvas.dataset.state = "ready";
      resize(true);
    } catch {
      fallback();
    }
  }

  function onPointerMove(event) {
    if (!running || event.pointerType === "touch") return;
    const rect = canvas.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;
    pointer.inside = x >= 0 && y >= 0 && x <= rect.width && y <= rect.height;
    pointer.x = (x - rect.width / 2) / Math.max(1, rect.height);
    pointer.y = y / Math.max(1, rect.height);
  }

  function onPointerLeave() {
    pointer.inside = false;
  }

  function onContextLost(event) {
    event.preventDefault();
    if (destroyed) return;
    lost = true;
    stopLoop();
    releaseResources();
    canvas.dataset.state = "lost";
  }

  function onContextRestored() {
    if (destroyed) return;
    lost = false;
    initialize();
  }

  function onPageHide(event) {
    if (!event.persisted) {
      destroy();
      return;
    }
    // bfcache keeps this controller alive, but not its animation or GL objects.
    suspended = true;
    stopLoop();
    releaseResources();
    clearCanvas();
    canvas.dataset.state = "fallback";
  }

  function onPageShow(event) {
    if (destroyed || !event.persisted || !suspended) return;
    suspended = false;
    initialize();
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    stopLoop();
    resizeObserver?.disconnect();
    intersection?.disconnect();
    window.removeEventListener("resize", onResize);
    window.removeEventListener("pagehide", onPageHide);
    window.removeEventListener("pageshow", onPageShow);
    document.removeEventListener("visibilitychange", synchronize);
    hero.removeEventListener("pointermove", onPointerMove);
    hero.removeEventListener("pointerleave", onPointerLeave);
    canvas.removeEventListener("webglcontextlost", onContextLost);
    canvas.removeEventListener("webglcontextrestored", onContextRestored);
    releaseResources();
    clearCanvas();
    canvas.dataset.state = "fallback";
  }

  function onResize() {
    resize();
  }

  const controller = {
    setPaused(value) {
      if (destroyed) return;
      paused = Boolean(value);
      synchronize();
    },
    setTheme(value) {
      if (destroyed) return;
      const next = value === "dark" ? "dark" : "light";
      if (next === theme) return;
      theme = next;
      updatePalette();
      render();
      synchronize();
    },
    destroy,
  };

  try {
    paused =
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    theme =
      document.documentElement.dataset.theme ||
      (window.matchMedia?.("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light");
    theme = theme === "dark" ? "dark" : "light";
    updatePalette();
    canvas.addEventListener("webglcontextlost", onContextLost);
    canvas.addEventListener("webglcontextrestored", onContextRestored);
    hero.addEventListener("pointermove", onPointerMove, { passive: true });
    hero.addEventListener("pointerleave", onPointerLeave);
    document.addEventListener("visibilitychange", synchronize);
    window.addEventListener("pagehide", onPageHide);
    window.addEventListener("pageshow", onPageShow);
    if ("ResizeObserver" in window) {
      resizeObserver = new ResizeObserver(onResize);
      resizeObserver.observe(canvas);
    } else {
      window.addEventListener("resize", onResize, { passive: true });
    }
    if ("IntersectionObserver" in window) {
      intersection = new IntersectionObserver(([entry]) => {
        visible = entry.isIntersecting;
        synchronize();
      });
      intersection.observe(hero);
    }
    // Without IntersectionObserver, retain the static first frame.
    initialize();
  } catch {
    destroy();
  }
  return controller;
}
