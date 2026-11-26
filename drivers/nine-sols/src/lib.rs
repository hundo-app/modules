//! Nine Sols: what Yi has gathered, read from the game's own flags. The save
//! only holds ids; their names and descriptions come from the game's text,
//! gathered once by `tools/extract.py` into `data/nine-sols.json`.

mod map;
mod save;

use std::collections::BTreeMap;

use hundo_sdk::{DriverError, Input, Item, Report, Section, Spoiler, Stat};
use serde::Deserialize;
use serde_json::Value;

struct NineSols;

impl hundo_sdk::Guest for NineSols {
    fn analyze(input: Input) -> Result<Report, DriverError> {
        analyze(&input)
    }
}

hundo_sdk::export!(NineSols);

/// One thing the save records: its flag, which field says it is done, and
/// its name in the game's own words.
#[derive(Deserialize)]
struct Entry {
    key: String,
    field: String,
    en: String,
    fr: String,
    en_details: Option<String>,
    fr_details: Option<String>,
    /// The shop that sells it, when one does.
    seller: Option<String>,
    /// The flags of handing it to someone, kept once it is gone.
    #[serde(default)]
    given: Vec<String>,
    /// Where it lies, in its scene.
    #[serde(default)]
    places: Vec<map::Place>,
    /// Its picture: a sprite of the game, drawn by Hundo.
    image: Option<String>,
}

const DATA: &str = include_str!("../data/nine-sols.json");

/// The sections, in the order the report shows them: id, group, English and
/// French titles, and how much each one gives away.
const SECTIONS: &[(&str, Group, &str, &str, Spoiler)] = &[
    ("seals", Group::Story, "Seals of the Sols", "Sceaux des Sols", Spoiler::Minor),
    ("key-items", Group::Story, "Key items", "Objets importants", Spoiler::Minor),
    ("nodes", Group::Story, "Root nodes", "Nœuds de racine", Spoiler::None),
    ("map-chips", Group::Story, "Map chips", "Puces de carte", Spoiler::None),
    ("jades", Group::Yi, "Jades", "Jades", Spoiler::None),
    ("skills", Group::Yi, "Skills", "Compétences", Spoiler::None),
    ("arrows", Group::Yi, "Arrows", "Flèches", Spoiler::None),
    ("computing", Group::Yi, "Computing units", "Unités de calcul", Spoiler::None),
    ("tools", Group::Yi, "Tools", "Outils", Spoiler::None),
    ("upgrades", Group::Yi, "Upgrade materials", "Matériaux d'amélioration", Spoiler::None),
    ("relics", Group::Collection, "Collectibles", "Objets de collection", Spoiler::None),
    ("poisons", Group::Collection, "Poisons", "Poisons", Spoiler::None),
    // The database, in the game's two tabs and with its words.
    ("characters", Group::Database, "Characters", "Personnages", Spoiler::Minor),
    ("info", Group::Database, "Info", "Info", Spoiler::Minor),
];

#[derive(Clone, Copy)]
enum Group {
    Story,
    Yi,
    Collection,
    Database,
}

impl Group {
    fn title(self, fr: bool) -> &'static str {
        match (self, fr) {
            (Group::Story, false) => "Journey",
            (Group::Story, true) => "Périple",
            (Group::Yi, false) => "Yi",
            (Group::Yi, true) => "Yi",
            (Group::Collection, false) => "Collection",
            (Group::Collection, true) => "Collection",
            // The game's own title for its database.
            (Group::Database, false) => "Collected Data",
            (Group::Database, true) => "Données collectées",
        }
    }
}

pub fn analyze(input: &Input) -> Result<Report, DriverError> {
    let fr = input.is_french();
    let t = |en: &'static str, fr_text: &'static str| if fr { fr_text } else { en };
    let file = input.files.iter().find(|f| !f.path.starts_with('@')).ok_or_else(|| DriverError::MissingFile("flags.sav".into()))?;
    let save = save::parse(&file.bytes)?;
    let data: BTreeMap<String, Vec<Entry>> = serde_json::from_str(DATA).expect("data/nine-sols.json is valid");

    let mut report = Report::empty();
    let p = &save.player;
    let num = |k: &str| p[k].as_f64().unwrap_or(0.0);
    report.stats = vec![
        Stat::new("playtime", t("Play time", "Temps de jeu"), hundo_sdk::format_duration(num("playTime"))),
        Stat::new("level", t("Level", "Niveau"), (num("level") as u64).to_string()),
        Stat::new("jin", t("Jin", "Jin"), (num("gold") as u64).to_string()),
        Stat::new("deaths", t("Deaths", "Morts"), (num("deathCount") as u64).to_string()),
        Stat::new("skill-points", t("Skill points left", "Points de compétence restants"), (num("skillPointLeft") as u64).to_string()),
    ];
    // After the last fight and the credits, the game puts the slot back to
    // its copy from before the point of no return (`saveslotN_
    // BeforeNoReturnPoint`), which knows nothing of the final run, and sets
    // `secondTimePlay`: the only trace the save keeps of it. What the final
    // run gave is then read from the account's Steam achievements, on such a
    // slot only: they are the account's, not this save's.
    let rolled_back = p["secondTimePlay"] == true;
    let steam = if rolled_back { achievements(input) } else { Vec::new() };
    let has = |id: &str| steam.iter().any(|a| a == id);
    let ending = if p["trueEndTriggered"] == true || has("Ach_Main_Good_End") {
        Some(t("True ending", "Vraie fin"))
    } else if p["badEndTriggered"] == true || has("Ach_Main_Bad_End") {
        Some(t("Normal ending", "Fin normale"))
    } else if p["finishedCreditRoll"] == true {
        Some(t("Credits seen", "Générique vu"))
    } else if rolled_back {
        Some(t("Final boss defeated", "Boss final vaincu"))
    } else {
        None
    };
    if let Some(e) = ending {
        report.stats.push(Stat::new("ending", t("Ending", "Fin"), e));
    }
    if rolled_back {
        report.warnings.push(
            t(
                "After the credits, Nine Sols puts the save back to where it was before the point of no return: \
                 whatever you did after it is no longer in the save. Hundo counts what your Steam achievements \
                 prove; the rest of the final run cannot be told.",
                "Après le générique, Nine Sols remet la sauvegarde à son état d'avant le point de non-retour : \
                 ce que tu as fait après n'y figure plus. Hundo compte ce que tes succès Steam prouvent ; \
                 le reste de la dernière partie ne peut pas être connu.",
            )
            .to_string(),
        );
    }

    let mut places = BTreeMap::new();
    for &(id, group, en, fr_title, spoiler) in SECTIONS {
        let entries = data.get(id).map(Vec::as_slice).unwrap_or_default();
        let items = items(id, entries, &save.flags, &steam, fr, spoiler, &mut places);
        if items.is_empty() {
            continue;
        }
        report.sections.push(Section::new(id, if fr { fr_title } else { en }, items).group(group.title(fr)));
    }
    report.maps.extend(map::build(input, &report.sections, &places, fr));
    Ok(report)
}

/// A section's items. Things the game names alike (each level of an
/// upgrade, a node reached from two sides) are one item, counted.
/// Where each item lies goes into `places`, by item id.
fn items(
    section: &str,
    entries: &[Entry],
    flags: &serde_json::Map<String, Value>,
    steam: &[String],
    fr: bool,
    spoiler: Spoiler,
    places: &mut BTreeMap<String, Vec<map::Place>>,
) -> Vec<Item> {
    let mut order: Vec<&str> = Vec::new();
    let mut by_name: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for e in entries {
        let name = if fr { e.fr.as_str() } else { e.en.as_str() };
        if !by_name.contains_key(name) {
            order.push(name);
        }
        by_name.entry(name).or_default().push(e);
    }
    order
        .into_iter()
        .map(|name| {
            let group = &by_name[name];
            let in_save = group.iter().filter(|e| is_done(section, e, flags)).count() as u32;
            let proven = group.iter().any(|e| proof(section, e).is_some_and(|a| steam.iter().any(|x| x == a)));
            let done = if proven { group.len() as u32 } else { in_save };
            let id = format!("{section}:{}", &group[0].key[..32]);
            let spots: Vec<map::Place> = group.iter().flat_map(|e| e.places.iter().cloned()).collect();
            if !spots.is_empty() {
                places.insert(id.clone(), spots);
            }
            // A node reached from two sides is one place: either side counts.
            let mut item = if section == "nodes" {
                Item::check(id, name, done > 0)
            } else if group.len() > 1 {
                Item::count(id, name, done, group.len() as u32)
            } else {
                Item::check(id, name, done > 0)
            };
            let details = if fr { group[0].fr_details.as_ref() } else { group[0].en_details.as_ref() };
            if let Some(d) = details {
                item = item.details(clean(d));
            }
            if let Some(image) = &group[0].image {
                item = item.image(format!("game:{image}"));
            }
            // Where it lies: the area, as the map names it, and its zone.
            if let Some(place) = group.iter().flat_map(|e| e.places.first()).next() {
                if let Some(zone) = map::zone(&place.scene, fr) {
                    item = item.areas([zone]);
                }
                if let Some(title) = map::title(&place.scene, fr) {
                    item = item.location(title);
                }
            }
            if proven && in_save < group.len() as u32 {
                item = item.hint(if fr {
                    "D'après tes succès Steam : la sauvegarde l'a perdu après le générique"
                } else {
                    "From your Steam achievements: the save lost it after the credits"
                });
            } else if group.iter().any(|e| e.seller.as_deref() == Some("chiyou")) {
                item = item.hint(if fr { "Vendu par Chiyou" } else { "Sold by Chiyou" });
            }
            item.spoiler(spoiler)
        })
        .collect()
}

/// Whether the save says it was got. The game clears `acquired` on what is
/// used up or given away (relics handed to Shuanshuan, poisons to Shennong,
/// map chips to Shanhai 9000), so the traces it leaves count too: the flag of
/// the hand-over, an item whose pick-up prompt was shown or that was seen in
/// the inventory, an arrow replaced by its next level (still `unlocked`).
fn is_done(section: &str, e: &Entry, flags: &serde_json::Map<String, Value>) -> bool {
    if e.given.iter().any(|k| flags.get(k).is_some_and(|f| f["acquired"] == true)) {
        return true;
    }
    let Some(f) = flags.get(&e.key) else { return false };
    let yes = |k: &str| f[k] == true;
    yes(&e.field)
        || yes("acquired")
        || f["ownNum"].as_i64().is_some_and(|n| n > 0)
        || (e.key.ends_with("ItemData") && (yes("viewed") || yes("promptViewed")))
        || (section == "arrows" && yes("unlocked"))
}

/// The Steam achievement that proves an entry was got, by the game's own id
/// for it. Each of these is still unset in the copy the game makes just
/// before the point of no return, so it can only come from the final run:
/// - defeating Eigong ("Learned From the Best") takes the way through the
///   Control Hub (`AG_ST_Hub`, its root node `SavePoint_Berserk` and its Root
///   Core monitor), and is when her two entries and the Root Stabilizer
///   (`裝備_古樹穩定器`, handed over by a script) come; Abacus's last entry
///   tells of her break-in at the Pavilion, the point of no return itself;
/// - the true ending ("Shooting Star") is the Rhizomatic Arrow fired at her,
///   Heng's memory (`看過A11妹妹回憶`, in the ending's own flags) and
///   Kuafu's last entry;
/// - the Friend Photo is picked up, and Shanhai 9000's last entry written,
///   as "Found You" unlocks;
/// - the jades and the arrows each have an achievement for the whole set.
fn proof(section: &str, e: &Entry) -> Option<&'static str> {
    const EIGONG: &str = "Ach_Main_A11_Finished";
    const TRUE_END: &str = "Ach_Main_Good_End";
    match (section, &e.key[..32]) {
        ("nodes", "da6613d2c8f7e4eb6ae1b4d0fe7fee93") => Some(EIGONG),
        ("info", "8acf8ff82f57b414887b68e127c7fee7") => Some(EIGONG),
        ("characters", "3d369fe813678e24a8d91c175a49505e" | "bcc8d5ccfad899043b0f7147ebe7a623" | "cfe769d63ebe1a3429f1838e2c401d1c") => Some(EIGONG),
        ("tools", "5776ed975228f4ecdb5ccbd2fec6c3bb") => Some(EIGONG),
        ("key-items", "40c579b4aaf773b4a9a9078633b1d7fb") => Some(TRUE_END),
        ("characters", "25ef42e93c6960741bd29655842bd29a" | "478420773cf1ab84dbb99aef2df8653e" | "72e94c0a930bb5a48a3de2e684cf3b81") => Some(TRUE_END),
        ("key-items", "a3960b34a56b2ad4998b5538bef3ce0b") => Some("Ach_Op_9000_Finished"),
        ("characters", "04d90cc08ac4b904d8d627052439857c" | "706f3711dce19e147a0e83f35d9f9024") => Some("Ach_Op_9000_Finished"),
        ("jades", _) => Some("Ach_Col_All_Jade"),
        ("arrows", _) => Some("Ach_Col_All_Arrow"),
        _ => None,
    }
}

/// The ids of the Steam achievements unlocked on the save's account, as the
/// host gives them (`reads-achievements` in driver.toml); none without Steam.
fn achievements(input: &Input) -> Vec<String> {
    input
        .files
        .iter()
        .find(|f| f.path == "@steam/achievements.json")
        .and_then(|f| serde_json::from_slice(&f.bytes).ok())
        .unwrap_or_default()
}

/// The game's descriptions carry rich-text tags for its own UI.
fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut tag = false;
    for c in text.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}
