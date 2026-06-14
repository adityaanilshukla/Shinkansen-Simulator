# Shinkansen Tokyo Drive

A first-person Shinkansen driving simulator. Drive an E5 Hayabusa along the
real Tohoku Shinkansen alignment from Tokyo Station north to Hasuda, through
WGS84-projected Tokyo geography sourced from OpenStreetMap (~130k real
building footprints with real heights, every named arterial, rivers, parks,
and the Imperial Palace moat). Written in Rust on Bevy 0.14.

Started as a port of `reference/e8-shinkansen-tokyo.html` (a single-file
Three.js demo) and grew from there. The reference HTML is still in the repo
for visual comparison.

## Running the game

### What you need

- A Rust toolchain. Install via [rustup](https://rustup.rs/) if you don't
  have it: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`.
  Any recent stable Rust (1.74+) works.
- Linux/macOS/Windows. On Linux you'll also need the usual ALSA / X11
  development headers for Bevy:

  ```bash
  # Debian / Ubuntu
  sudo apt install libasound2-dev libudev-dev pkg-config

  # Arch
  sudo pacman -S alsa-lib libxkbcommon

  # Fedora
  sudo dnf install alsa-lib-devel systemd-devel libxkbcommon-devel
  ```
- A GPU that can do Vulkan / Metal / DX12 — anything from the last ~10
  years. The scene has ~130k buildings + the viaduct + a 3D train model
  so an integrated GPU will work but will hitch in dense parts of Tokyo.

### Run

```bash
git clone https://github.com/<you>/Shinkansen-sim.git
cd Shinkansen-sim
cargo run --release
```

The first build takes a while — Bevy is a large dependency tree and the
generated `src/osm_data.rs` is ~31 MB of `&[(f32, f32)]` literals. Expect
5–15 minutes on a laptop. Subsequent builds are seconds.

You can also do `cargo run` (debug build) for faster iteration; the scene
runs ~30 FPS in debug, ~60–120 FPS in release.

The window opens directly into the cab in front of Tokyo Station — tap W
once to engage the throttle and you're off.

### Optional: regenerate the OSM city

The full OSM tile dumps are committed under `tools/` (about 115 MB total),
so the game compiles and runs out of the box without any external setup.
If you want to refresh the data or change the bbox covered, you can re-run
the extractor:

```bash
python3 tools/extract_osm.py
```

It reads every `tools/*.json` file (dedupes by OSM `@id`), classifies each
building from area + tagged height, and rewrites `src/osm_data.rs`. To
change the area covered, edit `TILE_BBOXES` in `extract_osm.py`, re-export
the matching GeoJSON tiles from https://overpass-turbo.eu (one query per
bbox), and re-run the script.

## Controls

| Key | Action |
| --- | --- |
| W / ↑ | Throttle lever up one notch (max +4) |
| S / ↓ | Throttle lever down one notch (min −4, brake) |
| C | Reverse train direction (also flips camera). Only when nearly stopped. |
| V | Reset camera (orbit + zoom back to chase) |
| E | Step off / board at stations |
| Mouse drag | Orbit camera |
| Wheel | Zoom |
| Esc | Exit |

HUD reads e.g. `FORWARD | PWR +3` or `STOPPED REV | BRK −2`.

In walk mode at a station: W/A/S/D move on the platform, mouse drag looks
around, E re-boards near either nose.

## Project layout

```
src/
├── main.rs              app + plugin wiring
│
├── route.rs             hand-coded Tohoku alignment + Catmull-Rom spline
├── spline.rs            spline math + arc-length sampling
├── geo.rs               WGS84 → world projection (1 unit ≈ 1 m)
│
├── track.rs             elevated viaduct (continuous ribbons), piers,
│                        parabolic arch spans, catenary masts + wire
├── stations.rs          platforms at Tokyo / Ueno / Omiya
│
├── tokyo.rs             OSM city: 130k extruded buildings, OSM roads,
│                        parks, water, facade textures, rooftop clutter,
│                        podium / balcony / awning bands
├── osm_data.rs          generated; baked OSM tile dump (~31 MB)
│
├── roads.rs             hand-coded Tokyo arterial polylines (Chuo-dori
│                        etc.) + RoadMask
├── water.rs             Sumida, Arakawa, Tokyo Bay + WaterMask
├── trees.rs             scattered trees + park trees
├── landmarks.rs         Tokyo Tower, Skytree, Mt. Fuji silhouette
├── lamps.rs             street lamps along arterials
│
├── sky.rs               clear color, ambient + directional light, sky dome
├── ground.rs            flat ground plane
├── minimap.rs           top-left mini-map UI
├── hud.rs               speed / direction / throttle HUD
├── camera.rs            follow camera (drive + walk modes)
├── audio.rs             procedural wind / brake sounds
├── input.rs             keyboard + mouse → Controls resource
│
├── train.rs             3-car GLB load + Car component
├── motion.rs            train-along-spline + banking
├── physics.rs           throttle lever, speed, drag, view-sign tracking
├── driver.rs            walk-around mode + driver figure
└── car.rs               procedural body fallback (currently unused)

assets/
└── train.glb            Sketchfab E5 Hayabusa, 3 cars merged

reference/
└── e8-shinkansen-tokyo.html   original Three.js demo (kept for comparison)

tools/
└── extract_osm.py       Python script that merges Overpass Turbo tile dumps
                         into src/osm_data.rs
```

## Data pipeline (OSM city)

The city geometry is baked into `src/osm_data.rs` (~31 MB of `&[(f32, f32)]`
literals) from raw Overpass Turbo dumps in `tools/*.json` (~115 MB). Both
the dumps and the generated Rust file are committed so the game compiles
and runs out of the box.

To regenerate the city — change the area covered, refresh stale OSM data,
or add new tiles:

1. Edit `TILE_BBOXES` in `tools/extract_osm.py` if you want a new bbox.
2. Open https://overpass-turbo.eu and run a query per bbox (the script's
   docstring has the Overpass query template). Export each as GeoJSON.
3. Save each export to `tools/<name>.json`.
4. `python3 tools/extract_osm.py` — merges every `*.json` in `tools/`,
   dedupes by OSM `@id`, classifies buildings by area + tagged height,
   and rewrites `src/osm_data.rs`.
5. `cargo build` to recompile against the new data.

## Known TODOs

Code-level TODOs in source (search `TODO(` to find them):

- **`tokyo.rs::TODO(buildings-on-roads)`** — edge-sampling against the OSM
  road mask catches most building/road overlaps, but some long, thin
  buildings still poke onto streets. Probably needs a sub-metre polygon
  rasterisation against the road mask, or a 0.5 m inset on each footprint
  before extruding.

- **`tokyo.rs::TODO(house-roofs)`** — every building gets a flat slab roof.
  An earlier attempt at pitched gable/hip roofs on short houses looked bad
  (AABB-aligned prisms over rotated/concave OSM polygons) so it was pulled.
  Should be redone polygon-aware (proper OBB, L/U shapes, hip vs gable per
  footprint).

- **`ground.rs::TODO(ground-color)`** — the ground plane keeps reading as
  blue regardless of ambient/sun tuning. The procedural noise texture has
  been stripped; it's now a plain warm-tan plane. Leads to investigate:
  - Linear-vs-sRGB conversion of the `AmbientLight::color` field.
  - The sky dome's contribution at low view angles (it's a 9 km unlit blue
    sphere centred on the camera).
  - No tone-mapping override on the StandardMaterial.

Visual/feature gaps reported during playthrough that haven't been addressed:

- **Track gaps on curves** (PARTIALLY FIXED) — the continuous-ribbon viaduct
  in `track.rs` is now seamless along the spline, but earlier versions had
  per-box rotation slivers on tight curves. If they reappear, the ribbon
  emission in `emit_ribbon` is the place to start.

- **Train is one rigid 3-car GLB** — on tight Tokyo bends the rear cars
  visibly leave the rail because the model doesn't bend. Fix would be to
  split the GLB in Blender into three per-car files (front cab + middle
  car + rear cab) and follow each car independently along the spline (the
  existing `OFFSETS` / `Car` machinery already supports per-car positioning).

- **Procedural audio is minimal** — only wind hiss + brake screech. No
  rolling rumble, no horn, no station chimes.

- **No timetable / mission** — the simulator runs free-form. Could add
  scripted runs (depart Tokyo, stop at Ueno, arrive Omiya by X:XX) with
  scoring on smoothness.

- **OSM coverage stops at Hasuda** — the route extends past the last tile.
  North of the last bbox the city goes empty (just procedural trees on the
  plain ground).

- **No people / vehicles** — the city is static. No pedestrians, no cars
  on the streets, no lights changing at intersections.

- **Walk mode is basic** — the driver figure is a stick-figure box, no
  animation, no interaction with the platform.

- **No weather / time-of-day** — fixed mid-day sun.

## Credits / sources

- Reference Three.js demo: `reference/e8-shinkansen-tokyo.html`.
- Train model: Sketchfab E5 Hayabusa, used under the model's license.
- City data: OpenStreetMap contributors, ODbL.
- Real Tokyo arterial polylines hand-extracted from Google Maps and OSM.
