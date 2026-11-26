//! The map of New Kunlun: each area's detailed map, drawn by Hundo from the
//! installed game inside its block of the game's world map (the layout is
//! `assets/map-layout.json`, made by `tools/extract.py`), with every tracked
//! thing at its spot.

use std::collections::BTreeMap;

use hundo_sdk::{GameMap, Input, MapMarker, Section};
use serde::Deserialize;

pub const ID: &str = "new-kunlun";

const AREAS: &str = include_str!("../data/areas.json");

/// The map Hundo drew: its size and its areas, in its pixels.
#[derive(Deserialize)]
struct Drawn {
    width: f32,
    height: f32,
    rooms: Vec<Room>,
}

#[derive(Deserialize)]
struct Room {
    scene: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// A spot in a scene, as fractions of its bounds from the left and the bottom.
#[derive(Deserialize, Clone)]
pub struct Place {
    pub scene: String,
    pub x: f32,
    pub y: f32,
    /// In a side room the map does not draw: at the door that leads there.
    #[serde(default)]
    pub approximate: bool,
}

#[derive(Deserialize)]
struct Title {
    en: String,
    fr: String,
}

/// An area's name, as the game's map names it.
pub fn title(scene: &str, fr: bool) -> Option<String> {
    let titles: BTreeMap<String, Title> = serde_json::from_str(AREAS).expect("data/areas.json is valid");
    titles.get(scene).map(|t| if fr { t.fr.clone() } else { t.en.clone() })
}

const ZONES: &str = include_str!("../data/zones.json");

/// The zone a scene is in (`A2_S1_…` is in the Power Reservoir), as the game
/// names it.
pub fn zone(scene: &str, fr: bool) -> Option<String> {
    let zones: BTreeMap<String, Title> = serde_json::from_str(ZONES).expect("data/zones.json is valid");
    zones.get(scene.split('_').next()?).map(|t| if fr { t.fr.clone() } else { t.en.clone() })
}

/// The map, when Hundo could draw it from the game: without the game
/// installed there is no picture to place things on.
pub fn build(input: &Input, sections: &[Section], places: &BTreeMap<String, Vec<Place>>, fr: bool) -> Option<GameMap> {
    let drawn: Drawn = input.file(&format!("@maps/{ID}.json")).ok().and_then(|f| serde_json::from_slice(&f.bytes).ok())?;
    let titles: BTreeMap<String, Title> = serde_json::from_str(AREAS).expect("data/areas.json is valid");
    let mut map = GameMap::new(ID, if fr { "La Nouvelle-Kunlun" } else { "New Kunlun" }, drawn.width, drawn.height);
    for room in &drawn.rooms {
        let label = titles.get(&room.scene).map(|t| if fr { t.fr.as_str() } else { t.en.as_str() }).unwrap_or(&room.scene);
        let (x0, y0, x1, y1) = (room.x, room.y, room.x + room.w, room.y + room.h);
        map = map.region(&format!("area:{}", room.scene), label, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    }
    for section in sections {
        for item in &section.items {
            let Some(spots) = places.get(&item.id) else { continue };
            for (n, p) in spots.iter().enumerate() {
                let Some(room) = drawn.rooms.iter().find(|r| r.scene == p.scene) else { continue };
                let (x, y) = (room.x + p.x * room.w, room.y + (1.0 - p.y) * room.h);
                let mut marker = MapMarker::at(format!("m-{}-{n}", item.id), item.title.clone(), x, y).item(item.id.clone()).category(section.title.clone());
                marker.approximate = p.approximate;
                map = map.marker(marker);
            }
        }
    }
    Some(map)
}
