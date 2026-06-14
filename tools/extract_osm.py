#!/usr/bin/env python3
"""Merge one or more GeoJSON tile dumps into a single Rust data file.

Run once, offline. The inputs are GeoJSON files saved from Overpass Turbo
(https://overpass-turbo.eu) and placed in this directory as `*.json`. Tiles
may overlap; entities with the same OSM `@id` are deduplicated.

The output `src/osm_data.rs` is committed to the repo, so the game has no
runtime dependency on the network or on Python.

Layout of the generated file:

    OSM_POINTS:     flat (f32, f32) array of world (x, z) coordinates
    OSM_BUILDINGS:  each indexes into OSM_POINTS, with class + height
    OSM_PARKS:      polygon rings of parks
    OSM_WATERS:     polygon rings or polylines of water
    OSM_ROADS:      polylines of roads

The world frame matches `src/geo.rs`: +x east, +z south, 1 unit = 1 m.
"""
from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path

# --- bbox + world projection ------------------------------------------------

# Hand-recorded bbox per tile (south_lat, west_lon, north_lat, east_lon). These
# match the Overpass Turbo queries that produced each tools/*.json. Lamps and
# the corridor mask read these so they only treat covered ground as "real".
TILE_BBOXES: list[tuple[float, float, float, float]] = [
    (35.66994, 139.75324, 35.69246, 139.78096),  # Tokyo
    (35.722, 139.738, 35.758, 139.782),          # Ueno / Tabata
    (35.792, 139.678, 35.828, 139.722),          # Akabane / Toda
    (35.862, 139.618, 35.918, 139.662),          # Saitama-Shintoshin / Omiya
    (35.962, 139.638, 36.018, 139.682),          # Hasuda
]

# Must match src/geo.rs exactly.
LAT0 = 35.680
LON0 = 139.739
X_PER_DEG_LON = 90_440.0
Z_PER_DEG_LAT = 110_900.0


def geo(lat: float, lon: float) -> tuple[float, float]:
    return ((lon - LON0) * X_PER_DEG_LON, -(lat - LAT0) * Z_PER_DEG_LAT)


def lonlat(c: list[float]) -> tuple[float, float]:
    """GeoJSON pair `[lon, lat]` -> world (x, z). Note the order."""
    return geo(c[1], c[0])


# --- building classification ------------------------------------------------

@dataclass
class Building:
    polygon: list[tuple[float, float]]
    height: float
    cls: str


CLASSES = ["House", "LowApt", "Apt", "Office", "Skyscraper", "Complex"]

ROAD_CLASSES = ["Major", "Primary", "Secondary", "Local"]


def road_class(highway_tag: str) -> str:
    if highway_tag in ("motorway", "trunk"):
        return "Major"
    if highway_tag == "primary":
        return "Primary"
    if highway_tag in ("secondary", "tertiary"):
        return "Secondary"
    return "Local"


def parse_height(tags: dict) -> float | None:
    h = tags.get("height")
    if h is not None:
        try:
            return float(str(h).replace("m", "").strip())
        except ValueError:
            pass
    levels = tags.get("building:levels")
    if levels is not None:
        try:
            return float(levels) * 3.3
        except ValueError:
            pass
    return None


def estimate_height(area: float, tags: dict) -> float:
    btype = (tags.get("building") or "").lower()
    if btype in ("house", "detached", "residential", "bungalow", "cabin"):
        return 5.5
    if btype in ("apartments", "dormitory", "terrace"):
        return 18.0
    if btype in ("office", "commercial", "retail"):
        return 24.0
    if btype in ("industrial", "warehouse"):
        return 10.0
    if btype in ("train_station", "station", "transportation", "hall"):
        return 14.0
    if area > 2000:
        return 28.0
    if area > 600:
        return 16.0
    return 7.0


def classify(area: float, height: float) -> str:
    if area > 2500:
        return "Complex"
    if height > 100:
        return "Skyscraper"
    if height > 50:
        return "Office"
    if area < 120 and height < 9:
        return "House"
    if area < 300 and height < 18:
        return "LowApt"
    return "Apt"


def polygon_area(ring: list[tuple[float, float]]) -> float:
    if len(ring) < 3:
        return 0.0
    a = 0.0
    n = len(ring)
    for i in range(n):
        x1, z1 = ring[i]
        x2, z2 = ring[(i + 1) % n]
        a += x1 * z2 - x2 * z1
    return abs(a) * 0.5


def ring_from_coords(coords: list[list[float]]) -> list[tuple[float, float]]:
    """Convert a GeoJSON ring into a closed-vs-open-agnostic world ring."""
    ring = [lonlat(p) for p in coords]
    if len(ring) > 2 and ring[0] == ring[-1]:
        ring = ring[:-1]
    return ring


# --- emit Rust --------------------------------------------------------------

def emit_rust(
    buildings: list[Building],
    parks: list[list[tuple[float, float]]],
    water_polygons: list[list[tuple[float, float]]],
    water_lines: list[list[tuple[float, float]]],
    roads: list[tuple[list[tuple[float, float]], str]],
    out: Path,
) -> None:
    points: list[tuple[float, float]] = []

    def push(ring: list[tuple[float, float]]) -> tuple[int, int]:
        start = len(points)
        points.extend(ring)
        return (start, len(ring))

    building_records: list[tuple[int, int, str, float]] = []
    for b in buildings:
        start, count = push(b.polygon)
        building_records.append((start, count, b.cls, b.height))

    park_records = [push(p) for p in parks]
    water_poly_records = [push(w) for w in water_polygons]
    water_line_records = [push(w) for w in water_lines]
    road_records: list[tuple[int, int, str]] = []
    for line, cls in roads:
        s, c = push(line)
        road_records.append((s, c, cls))

    print(
        f"emit: buildings={len(building_records)} parks={len(park_records)} "
        f"water_polys={len(water_poly_records)} water_lines={len(water_line_records)} "
        f"roads={len(road_records)} points={len(points)}",
        file=sys.stderr,
    )

    lines: list[str] = []
    w = lines.append

    w("//! Auto-generated by tools/extract_osm.py. Do not edit.")
    w("//!")
    w(f"//! Source: OpenStreetMap via Overpass Turbo, {len(TILE_BBOXES)} tile(s).")
    w(f"//! Counts: {len(building_records)} buildings, {len(park_records)} parks, "
      f"{len(water_poly_records)} water polygons, {len(water_line_records)} water lines, "
      f"{len(road_records)} roads.")
    w("")
    w("#![allow(dead_code)]")
    w("")
    w("use crate::tokyo::{BuildingClass, OsmBuilding, OsmRing, OsmRoad, RoadClass};")
    w("")

    # Tile bboxes in WORLD coords (min_x, min_z, max_x, max_z). World z is
    # flipped from latitude, so the south lat maps to a larger z and vice versa.
    w(f"pub static OSM_TILE_BBOXES: [(f32, f32, f32, f32); {len(TILE_BBOXES)}] = [")
    for (s_lat, w_lon, n_lat, e_lon) in TILE_BBOXES:
        min_x, max_z = geo(s_lat, w_lon)
        max_x, min_z = geo(n_lat, e_lon)
        w(f"    ({min_x:.2f}, {min_z:.2f}, {max_x:.2f}, {max_z:.2f}),")
    w("];")
    w("")

    w(f"pub static OSM_POINTS: [(f32, f32); {len(points)}] = [")
    for x, z in points:
        w(f"    ({x:.2f}, {z:.2f}),")
    w("];")
    w("")

    w(f"pub static OSM_BUILDINGS: [OsmBuilding; {len(building_records)}] = [")
    for start, count, cls, h in building_records:
        w(
            f"    OsmBuilding {{ start: {start}, count: {count}, "
            f"class: BuildingClass::{cls}, height: {h:.2f} }},"
        )
    w("];")
    w("")

    w(f"pub static OSM_PARKS: [OsmRing; {len(park_records)}] = [")
    for s, c in park_records:
        w(f"    OsmRing {{ start: {s}, count: {c} }},")
    w("];")
    w("")

    w(f"pub static OSM_WATER_POLYGONS: [OsmRing; {len(water_poly_records)}] = [")
    for s, c in water_poly_records:
        w(f"    OsmRing {{ start: {s}, count: {c} }},")
    w("];")
    w("")

    w(f"pub static OSM_WATER_LINES: [OsmRing; {len(water_line_records)}] = [")
    for s, c in water_line_records:
        w(f"    OsmRing {{ start: {s}, count: {c} }},")
    w("];")
    w("")

    w(f"pub static OSM_ROADS: [OsmRoad; {len(road_records)}] = [")
    for s, c, cls in road_records:
        w(f"    OsmRoad {{ start: {s}, count: {c}, class: RoadClass::{cls} }},")
    w("];")
    w("")

    out.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {out} ({out.stat().st_size // 1024} KiB)", file=sys.stderr)


# --- main -------------------------------------------------------------------

def main() -> None:
    src_dir = Path(__file__).parent
    tiles = sorted(p for p in src_dir.glob("*.json"))
    if not tiles:
        print(f"no *.json tiles in {src_dir}; run Overpass Turbo first", file=sys.stderr)
        sys.exit(1)

    feats: list[dict] = []
    seen: set[str] = set()
    for tile in tiles:
        data = json.loads(tile.read_text(encoding="utf-8"))
        tile_feats = data.get("features", [])
        kept = 0
        for f in tile_feats:
            oid = (f.get("properties") or {}).get("@id")
            if oid is not None and oid in seen:
                continue
            if oid is not None:
                seen.add(oid)
            feats.append(f)
            kept += 1
        print(f"  {tile.name}: {kept}/{len(tile_feats)} new features", file=sys.stderr)
    print(f"merged {len(feats)} unique features from {len(tiles)} tile(s)", file=sys.stderr)

    buildings: list[Building] = []
    parks: list[list[tuple[float, float]]] = []
    water_polygons: list[list[tuple[float, float]]] = []
    water_lines: list[list[tuple[float, float]]] = []
    roads: list[tuple[list[tuple[float, float]], str]] = []  # (line, class)

    for f in feats:
        props = f.get("properties") or {}
        geom = f.get("geometry") or {}
        gtype = geom.get("type")
        coords = geom.get("coordinates")
        if not coords:
            continue

        if "building" in props:
            rings = []
            if gtype == "Polygon":
                rings.append(ring_from_coords(coords[0]))
            elif gtype == "MultiPolygon":
                for poly in coords:
                    rings.append(ring_from_coords(poly[0]))
            for ring in rings:
                if len(ring) < 3:
                    continue
                area = polygon_area(ring)
                if area < 10.0:
                    continue
                h = parse_height(props)
                if h is None:
                    h = estimate_height(area, props)
                buildings.append(
                    Building(polygon=ring, height=h, cls=classify(area, h))
                )
            continue

        if props.get("leisure") in ("park", "garden") and gtype == "Polygon":
            ring = ring_from_coords(coords[0])
            if len(ring) >= 3:
                parks.append(ring)
            continue

        if props.get("natural") == "water" and gtype == "Polygon":
            ring = ring_from_coords(coords[0])
            if len(ring) >= 3:
                water_polygons.append(ring)
            continue

        if props.get("waterway") in ("river", "canal") and gtype == "LineString":
            line = [lonlat(p) for p in coords]
            if len(line) >= 2:
                water_lines.append(line)
            continue

        if props.get("highway") and gtype == "LineString":
            line = [lonlat(p) for p in coords]
            if len(line) >= 2:
                roads.append((line, road_class(props["highway"])))
            continue

    print(
        f"classified: {len(buildings)} buildings, {len(parks)} parks, "
        f"{len(water_polygons)} water polys, {len(water_lines)} water lines, "
        f"{len(roads)} roads",
        file=sys.stderr,
    )

    histogram: dict[str, int] = {c: 0 for c in CLASSES}
    for b in buildings:
        histogram[b.cls] += 1
    print("class histogram: " + ", ".join(f"{k}={v}" for k, v in histogram.items()),
          file=sys.stderr)

    out = Path(__file__).parent.parent / "src" / "osm_data.rs"
    emit_rust(buildings, parks, water_polygons, water_lines, roads, out)


if __name__ == "__main__":
    main()
