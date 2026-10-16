// Floating 3D keycaps for the hero. Loaded lazily; the page works without it.
import * as THREE from 'three';
import { RoundedBoxGeometry } from 'three/examples/jsm/geometries/RoundedBoxGeometry.js';
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js';

const CORAL = 0xf0535f;
const CAP = 0xe8364b;

function legendTexture(draw, w, h) {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  const g = c.getContext('2d');
  draw(g, w, h);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  t.anisotropy = 8;
  return t;
}

function bolt(g, w, h) {
  // the app icon's bolt, scaled from its 1024 grid
  const s = Math.min(w, h) / 760, ox = (w - 760 * s) / 2, oy = (h - 760 * s) / 2;
  const pts = [[594, 163], [307, 553], [481, 553], [420, 859], [716, 450], [543, 450]];
  g.fillStyle = '#ffffff';
  g.beginPath();
  pts.forEach(([x, y], i) => {
    const px = ox + (x - 280) * s * 1.05, py = oy + (y - 130) * s * 1.0;
    i ? g.lineTo(px, py) : g.moveTo(px, py);
  });
  g.closePath();
  g.fill();
}

function text(label, size) {
  return (g, w, h) => {
    g.fillStyle = '#f4f4f6';
    g.font = `500 ${size}px "Stack Sans Headline", "Helvetica Neue", system-ui, sans-serif`;
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillText(label, w / 2, h / 2 + size * 0.04);
  };
}

export function init(canvas, { reduced = false } = {}) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: 'high-performance' });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 0.85;

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environmentIntensity = 0.55;

  const camera = new THREE.PerspectiveCamera(28, 1, 0.1, 100);
  camera.position.set(0, 1.2, 21);

  const rim = new THREE.PointLight(CORAL, 90, 30, 1.6);
  rim.position.set(-3, -3, 4);
  scene.add(rim);
  const key = new THREE.DirectionalLight(0xffffff, 1.4);
  key.position.set(4, 8, 6);
  scene.add(key);

  const FLOOR = -3.9;
  const group = new THREE.Group();
  scene.add(group);
  const mirrorRoot = new THREE.Group();
  mirrorRoot.scale.y = -1;
  mirrorRoot.position.y = 2 * FLOOR;
  const mirror = new THREE.Group();
  mirrorRoot.add(mirror);
  scene.add(mirrorRoot);

  // the app icon's bolt as real geometry
  function boltShape() {
    const pts = [[594, 163], [307, 553], [481, 553], [420, 859], [716, 450], [543, 450]];
    const k = 2.15 / 696;
    const sh = new THREE.Shape();
    pts.forEach(([x, y], i) => {
      const px = (x - 511.5) * k, py = -(y - 511) * k;
      i ? sh.lineTo(px, py) : sh.moveTo(px, py);
    });
    sh.closePath();
    return sh;
  }
  const boltGeo = new THREE.ExtrudeGeometry(boltShape(), { depth: 0.2, bevelEnabled: true, bevelThickness: 0.06, bevelSize: 0.06, bevelSegments: 4, curveSegments: 1 });

  function build(parent, ghost) {
    const list = [];
    const op = ghost ? 0.32 : 1;
    function makeKey({ w, d, h = 1, color, legend, lw = 512, lh = 512, pos, rot = [0, 0, 0], phase = 0, metal = 0.15, isBolt = false }) {
      const root = new THREE.Group();
      const mat = new THREE.MeshPhysicalMaterial({ color, roughness: 0.46, metalness: metal, clearcoat: ghost ? 0 : 1, clearcoatRoughness: 0.22, transparent: ghost, opacity: op, depthWrite: !ghost });
      const body = new THREE.Mesh(new RoundedBoxGeometry(w, h, d, 8, 0.26), mat);
      root.add(body);
      if (isBolt) {
        const bm = new THREE.Mesh(boltGeo, new THREE.MeshPhysicalMaterial({ color: 0xffffff, roughness: 0.2, metalness: 0, clearcoat: 1, emissive: 0xffffff, emissiveIntensity: ghost ? 0 : 0.12, transparent: ghost, opacity: op, depthWrite: !ghost }));
        bm.rotation.x = -Math.PI / 2;
        bm.position.y = h / 2 - 0.02;
        root.add(bm);
      } else {
        const plate = new THREE.Mesh(
          new THREE.PlaneGeometry(w * 0.78, d * 0.78),
          new THREE.MeshStandardMaterial({ map: legendTexture(legend, lw, lh), transparent: true, opacity: op, roughness: 0.5, metalness: 0, depthWrite: false })
        );
        plate.rotation.x = -Math.PI / 2;
        plate.position.y = h / 2 + 0.004;
        root.add(plate);
      }
      root.position.set(...pos);
      root.rotation.set(...rot);
      root.userData = { base: new THREE.Vector3(...pos), baseRot: new THREE.Euler(...rot), phase, press: 0, body };
      parent.add(root);
      list.push(root);
    }
    makeKey({ w: 3.4, d: 3.4, h: 1.15, color: CAP, legend: null, isBolt: true, pos: [0.9, 1.0, 0], rot: [0, -0.35, 0], phase: 0, metal: 0.05 });
    makeKey({ w: 2.5, d: 2.5, h: 1, color: 0x16161a, legend: text('⌥', 300), pos: [-3.2, -0.9, 1.2], rot: [0, 0.4, 0.05], phase: 1.7, metal: 0.2 });
    makeKey({ w: 5.6, d: 2.5, h: 1, color: 0x16161a, legend: text('Space', 150), lw: 1024, lh: 460, pos: [2.0, -2.6, 1.6], rot: [0, -0.28, -0.04], phase: 3.1, metal: 0.2 });
    return list;
  }
  const keys = build(group, false);
  const ghosts = build(mirror, true);

  // coral dust
  const N = 160, pos = new Float32Array(N * 3), seed = [];
  for (let i = 0; i < N; i++) {
    pos[i * 3] = (Math.random() - 0.5) * 16;
    pos[i * 3 + 1] = (Math.random() - 0.5) * 10;
    pos[i * 3 + 2] = (Math.random() - 0.5) * 8;
    seed.push(Math.random() * 6.28);
  }
  const dust = new THREE.Points(
    new THREE.BufferGeometry().setAttribute('position', new THREE.BufferAttribute(pos, 3)),
    new THREE.PointsMaterial({ color: CORAL, size: 0.05, transparent: true, opacity: 0.7, depthWrite: false })
  );
  scene.add(dust);

  group.rotation.set(0.62, -0.5, 0.06);
  group.scale.setScalar(0.86);
  group.userData.y0 = 0.5;
  mirror.scale.setScalar(0.86);

  const mouse = { x: 0, y: 0, tx: 0, ty: 0 };
  const parent = canvas.parentElement;
  window.addEventListener('pointermove', (e) => {
    const r = parent.getBoundingClientRect();
    mouse.tx = ((e.clientX - r.left) / r.width - 0.5) * 2;
    mouse.ty = ((e.clientY - r.top) / r.height - 0.5) * 2;
  }, { passive: true });

  // click a key: it sinks and springs back
  const ray = new THREE.Raycaster();
  canvas.addEventListener('pointerdown', (e) => {
    const r = canvas.getBoundingClientRect();
    const v = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    ray.setFromCamera(v, camera);
    const hit = ray.intersectObjects(keys.map((k) => k.userData.body), false)[0];
    if (hit) keys.find((k) => k.userData.body === hit.object).userData.press = 1;
  });
  canvas.style.cursor = 'pointer';

  function resize() {
    const w = parent.clientWidth, h = parent.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.fov = w / h < 0.9 ? 36 : 28;
    camera.updateProjectionMatrix();
  }
  new ResizeObserver(resize).observe(parent);
  resize();

  let visible = true, raf = 0;
  new IntersectionObserver((en) => { visible = en[0].isIntersecting; if (visible && !raf && !reduced) loop(); }, { threshold: 0 }).observe(parent);

  const clock = new THREE.Clock();
  function frame() {
    const t = clock.getElapsedTime();
    mouse.x += (mouse.tx - mouse.x) * 0.05;
    mouse.y += (mouse.ty - mouse.y) * 0.05;
    const sc = Math.min(window.scrollY / 700, 1);
    group.rotation.y = -0.5 + mouse.x * 0.28 + sc * 0.5;
    group.rotation.x = 0.62 - mouse.y * 0.16 + sc * 0.25;
    group.position.y = group.userData.y0 + sc * 1.2;
    for (const k of keys) {
      const u = k.userData;
      u.press *= 0.9;
      const bob = reduced ? 0 : Math.sin(t * 0.9 + u.phase) * 0.18;
      k.position.set(u.base.x, u.base.y + bob - u.press * 0.5, u.base.z);
      k.rotation.set(u.baseRot.x + (reduced ? 0 : Math.sin(t * 0.6 + u.phase) * 0.05), u.baseRot.y + (reduced ? 0 : Math.cos(t * 0.5 + u.phase) * 0.07), u.baseRot.z);
    }
    mirror.rotation.copy(group.rotation); mirror.position.copy(group.position);
    keys.forEach((k, i) => { ghosts[i].position.copy(k.position); ghosts[i].rotation.copy(k.rotation); });
    const a = dust.geometry.attributes.position;
    for (let i = 0; i < N; i++) a.array[i * 3 + 1] += Math.sin(t * 0.4 + seed[i]) * 0.0016;
    a.needsUpdate = true;
    dust.rotation.y = t * 0.02;
    renderer.render(scene, camera);
  }
  function loop() {
    raf = 0;
    if (!visible || document.hidden) return;
    frame();
    raf = requestAnimationFrame(loop);
  }
  document.addEventListener('visibilitychange', () => { if (!document.hidden && !raf && visible && !reduced) loop(); });
  frame();
  if (!reduced) loop();
  return true;
}
