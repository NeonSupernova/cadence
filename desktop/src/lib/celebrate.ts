// Small, dependency-free celebration effects: confetti bursts and a soft chime.
// Everything is skipped when the system asks to reduce motion.

const COLORS = ['#7c5cff', '#14c8b0', '#ff5ca8', '#ffb938', '#3fa9ff', '#b36bff'];
// Shapes: 0 ribbon, 1 dot, 2 star, 3 glowing four-point twinkle that floats.
type Particle = { x: number; y: number; vx: number; vy: number; size: number; color: string; spin: number; angle: number; life: number; shape: 0 | 1 | 2 | 3; gravity: number; phase: number };

let canvas: HTMLCanvasElement | null = null;
let particles: Particle[] = [];
let frame = 0;

export const reducedMotion = () =>
  typeof window !== 'undefined' &&
  (document.documentElement.dataset.reduceMotion === 'true' || window.matchMedia?.('(prefers-reduced-motion: reduce)').matches);

const SOUND_KEY = 'cadence.celebrationSounds';
export function soundsEnabled(): boolean {
  try { return localStorage.getItem(SOUND_KEY) !== 'off'; } catch { return true; }
}
export function setSoundsEnabled(on: boolean) {
  try { localStorage.setItem(SOUND_KEY, on ? 'on' : 'off'); } catch { /* storage unavailable */ }
}

function surface(): CanvasRenderingContext2D | null {
  if (!canvas) {
    canvas = document.createElement('canvas');
    canvas.setAttribute('aria-hidden', 'true');
    canvas.className = 'celebration-canvas';
    document.body.appendChild(canvas);
  }
  const ratio = window.devicePixelRatio || 1;
  if (canvas.width !== innerWidth * ratio || canvas.height !== innerHeight * ratio) {
    canvas.width = innerWidth * ratio;
    canvas.height = innerHeight * ratio;
  }
  const ctx = canvas.getContext('2d');
  ctx?.setTransform(ratio, 0, 0, ratio, 0, 0);
  return ctx;
}

function loop() {
  const ctx = surface();
  if (!ctx || !canvas) return;
  ctx.clearRect(0, 0, innerWidth, innerHeight);
  particles = particles.filter(p => p.life > 0 && p.y < innerHeight + 40);
  for (const p of particles) {
    p.vy += p.gravity; p.vx *= 0.985; p.vy *= 0.985;
    p.x += p.vx; p.y += p.vy; p.angle += p.spin; p.life -= 1;
    ctx.save();
    ctx.globalAlpha = Math.min(1, p.life / 30) * (p.shape === 3 ? 0.55 + 0.45 * Math.sin(p.life * 0.35 + p.phase) : 1);
    ctx.translate(p.x, p.y); ctx.rotate(p.angle);
    ctx.fillStyle = p.color;
    if (p.shape === 0) ctx.fillRect(-p.size / 2, -p.size / 4, p.size, p.size / 2);
    else if (p.shape === 1) { ctx.beginPath(); ctx.arc(0, 0, p.size / 2.6, 0, Math.PI * 2); ctx.fill(); }
    else if (p.shape === 3) {
      const r = p.size, w = p.size / 4.5;
      ctx.shadowColor = p.color; ctx.shadowBlur = 10;
      ctx.beginPath(); ctx.moveTo(0, -r); ctx.quadraticCurveTo(w, -w, r, 0); ctx.quadraticCurveTo(w, w, 0, r); ctx.quadraticCurveTo(-w, w, -r, 0); ctx.quadraticCurveTo(-w, -w, 0, -r); ctx.fill();
      ctx.fillStyle = '#fff'; ctx.beginPath(); ctx.arc(0, 0, w * .7, 0, Math.PI * 2); ctx.fill();
    }
    else { ctx.beginPath(); for (let i = 0; i < 5; i++) { const a = (i * 4 * Math.PI) / 5 - Math.PI / 2; ctx.lineTo(Math.cos(a) * p.size / 1.8, Math.sin(a) * p.size / 1.8); } ctx.closePath(); ctx.fill(); }
    ctx.restore();
  }
  frame = particles.length ? requestAnimationFrame(loop) : 0;
  if (!particles.length) ctx.clearRect(0, 0, innerWidth, innerHeight);
}

function spawn(x: number, y: number, count: number, power: number, spread = Math.PI * 2, direction = -Math.PI / 2) {
  for (let i = 0; i < count; i++) {
    const angle = direction + (Math.random() - 0.5) * spread;
    const speed = power * (0.45 + Math.random() * 0.75);
    particles.push({ x, y, vx: Math.cos(angle) * speed, vy: Math.sin(angle) * speed, size: 6 + Math.random() * 7,
      color: COLORS[Math.floor(Math.random() * COLORS.length)], spin: (Math.random() - 0.5) * 0.4, angle: Math.random() * Math.PI,
      life: 70 + Math.random() * 50, shape: (Math.random() < 0.18 ? 3 : Math.floor(Math.random() * 3)) as Particle['shape'], gravity: 0.32, phase: Math.random() * 6 });
  }
  if (!frame) frame = requestAnimationFrame(loop);
}

/** A ring of floating twinkles, for starting something. Lighter than confetti. */
function twinkles(x: number, y: number, count: number, power: number, colors = COLORS) {
  for (let i = 0; i < count; i++) {
    const angle = (i / count) * Math.PI * 2 + Math.random() * 0.4;
    const speed = power * (0.5 + Math.random() * 0.6);
    particles.push({ x, y, vx: Math.cos(angle) * speed, vy: Math.sin(angle) * speed - 0.6, size: 4 + Math.random() * 6,
      color: colors[Math.floor(Math.random() * colors.length)], spin: (Math.random() - 0.5) * 0.08, angle: 0,
      life: 45 + Math.random() * 35, shape: 3, gravity: -0.015, phase: Math.random() * 6 });
  }
  if (!frame) frame = requestAnimationFrame(loop);
}

let audio: AudioContext | null = null;
function chime(notes: number[], gap = 0.07) {
  if (!soundsEnabled()) return;
  try {
    audio ??= new AudioContext();
    const start = audio.currentTime + 0.01;
    notes.forEach((freq, i) => {
      const osc = audio!.createOscillator(); const gain = audio!.createGain();
      osc.type = 'sine'; osc.frequency.value = freq;
      const t = start + i * gap;
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.08, t + 0.015);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.35);
      osc.connect(gain).connect(audio!.destination);
      osc.start(t); osc.stop(t + 0.4);
    });
  } catch { /* audio unavailable */ }
}

/** A small pop at a point: checking off one step. */
export function burst(x: number, y: number) {
  chime([784, 1175], 0.05);
  if (reducedMotion()) return;
  spawn(x, y, 26, 7);
}

/** Every step in a task is done. */
export function allSteps(x: number, y: number) {
  chime([659, 784, 988, 1319], 0.06);
  if (reducedMotion()) return;
  spawn(x, y, 60, 10);
}

/** A whole task is complete: confetti from both lower corners. */
export function celebrate() {
  chime([523, 659, 784, 1047, 1319], 0.08);
  if (reducedMotion()) return;
  spawn(0, innerHeight, 90, 19, Math.PI / 3, -Math.PI / 3);
  spawn(innerWidth, innerHeight, 90, 19, Math.PI / 3, (-2 * Math.PI) / 3);
  setTimeout(() => spawn(innerWidth / 2, innerHeight * 0.35, 70, 11), 220);
}

/** Center of an element, for anchoring a burst. */
/** Starting a timer: a quick shimmer in the task's color. */
export function sparkle(x: number, y: number, hue?: string) {
  chime([1047, 1568], 0.04);
  if (reducedMotion()) return;
  twinkles(x, y, 14, 3.2, hue ? [hue, hue, '#ffffff', '#ffb938'] : COLORS);
}

/** A soft two-note bell for break reminders. */
export function nudge() {
  chime([880, 1320, 880], 0.16);
}

export function centerOf(el: Element | null | undefined): [number, number] {
  const r = el?.getBoundingClientRect();
  return r ? [r.left + r.width / 2, r.top + r.height / 2] : [innerWidth / 2, innerHeight / 2];
}
