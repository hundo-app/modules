//! Hades: reads `ProfileN.sav` and reports every arm and Aspect, the Fated
//! List, the Mirror of Night, each relationship with its keepsake and
//! companion, and the House Contractor's works.
//!
//! What there is to get comes from the game's own scripts (`data/hades.json`,
//! built by `tools/extract.py`, identifiers only); the names, from the
//! reader's copy of the game in their language (`HelpText`), English
//! filling the gaps.

mod save;

use std::collections::HashMap;

use hundo_sdk::{DriverError, Input, Item, Report, Section, Spoiler, Stat, Status, format_duration};
use save::Lua;
use serde::Deserialize;

struct Hades;

impl hundo_sdk::Guest for Hades {
    fn analyze(input: Input) -> Result<Report, DriverError> {
        analyze(&input)
    }
}

hundo_sdk::export!(Hades);

const DATA: &str = include_str!("../data/hades.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    weapons: Vec<Weapon>,
    quests: Vec<String>,
    gifts: HashMap<String, Gift>,
    gift_ordering: Vec<String>,
    mirror: Vec<Vec<Talent>>,
    contractor: Vec<Work>,
}

#[derive(Deserialize)]
struct Weapon {
    id: String,
    aspects: Vec<Aspect>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aspect {
    #[serde(rename = "trait")]
    name: String,
    max: u32,
    starts_unlocked: bool,
    /// Nyx's line (or another's) that reveals a hidden Aspect.
    revealed_by: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Gift {
    maximum: Option<u32>,
    locked: Option<u32>,
    keepsake: Option<String>,
    keepsake_at: Option<u32>,
    companion: Option<String>,
    companion_at: Option<u32>,
}

#[derive(Deserialize)]
struct Talent {
    id: String,
    /// Its name in the game's text is its icon's.
    icon: String,
    max: u32,
}

#[derive(Deserialize)]
struct Work {
    id: String,
    slot: String,
}

/// Guardians by the name the game counts their kills under, and their realm.
const GUARDIANS: [(&str, &str); 7] = [
    ("Harpy", "Tartarus"),
    ("Harpy2", "Tartarus"),
    ("Harpy3", "Tartarus"),
    ("HydraHeadImmortal", "Asphodel"),
    ("Theseus", "Elysium"),
    ("Minotaur", "Elysium"),
    ("Hades", "Styx"),
];

/// The realms from the House up, by the id runs record them under.
const REALMS: [(&str, &str, &str); 5] = [
    ("Tartarus", "Tartare", "Tartarus"),
    ("Asphodel", "Asphodèle", "Asphodel"),
    ("Elysium", "Élysée", "Elysium"),
    ("Styx", "Temple du Styx", "Temple of Styx"),
    ("Surface", "Surface", "Surface"),
];

/// Chambers a keepsake must be worn through to rank up: rank 2, then rank 3.
const KEEPSAKE_RANKS: [u32; 2] = [25, 75];

/// The game's names, by their id, and the English ones for wiki links.
struct Words {
    names: HashMap<String, String>,
    english: HashMap<String, String>,
}

impl Words {
    /// `HelpText` in the reader's language over the English one. A name
    /// that is a keyword's (`{$Keywords.AlphaAttack}`) is that keyword's.
    fn read(input: &Input) -> Self {
        let mut names = HashMap::new();
        let mut english = HashMap::new();
        let mut aliases = Vec::new();
        let texts: Vec<_> = input.files_under("@texts/").collect();
        for f in texts.iter().filter(|f| f.path.contains("/en/")) {
            let (n, a) = sjson_names(&f.bytes);
            english.extend(n);
            aliases.extend(a);
        }
        names.extend(english.clone());
        for f in texts.iter().filter(|f| !f.path.contains("/en/")) {
            let (n, a) = sjson_names(&f.bytes);
            names.extend(n);
            aliases.extend(a);
        }
        for (id, keyword) in aliases {
            for map in [&mut names, &mut english] {
                if let Some(name) = map.get(&keyword).cloned() {
                    map.entry(id.clone()).or_insert(name);
                }
            }
        }
        Words { names, english }
    }

    fn get(&self, id: &str) -> Option<String> {
        self.names.get(id).cloned()
    }

    /// The game's name, or a fallback when the game's text could not be read.
    fn or(&self, id: &str, fallback: &str) -> String {
        self.get(id).unwrap_or_else(|| fallback.to_string())
    }

    /// A character's name: `NPC_Persephone_Home_01` is named as `NPC_Persephone_01`.
    fn character(&self, id: &str) -> String {
        self.get(id).or_else(|| self.get(&id.replace("_Home", ""))).unwrap_or_else(|| id.to_string())
    }

    /// The Hades wiki's page for a thing, by its English name.
    fn wiki(&self, id: &str) -> Option<String> {
        self.english.get(id).map(|n| format!("https://hades.fandom.com/wiki/{}", n.replace(' ', "_")))
    }
}

/// An id and a name, or an id and the keyword it is named after.
type Pair = (String, String);

/// `Id = "SwordWeapon"` then `DisplayName = "Stygius"`: the names of a
/// `.sjson` text file, the game's markup (`{#…}`, `{!Icons…}`) taken out,
/// and the names that are only a keyword's, as (id, keyword).
fn sjson_names(bytes: &[u8]) -> (Vec<Pair>, Vec<Pair>) {
    let text = String::from_utf8_lossy(bytes);
    let mut names = Vec::new();
    let mut aliases = Vec::new();
    let mut id: Option<String> = None;
    for line in text.lines().map(str::trim) {
        if let Some(v) = line.strip_prefix("Id = ").and_then(quoted) {
            id = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("DisplayName = ").and_then(quoted) {
            let Some(i) = id.take() else { continue };
            if let Some(keyword) = v.strip_prefix("{$Keywords.").and_then(|k| k.strip_suffix('}')) {
                aliases.push((i, keyword.to_string()));
                continue;
            }
            let clean = strip_markup(v);
            if !clean.is_empty() {
                names.push((i, clean));
            }
        } else if line == "}" {
            id = None;
        }
    }
    (names, aliases)
}

/// The text between the first quotes, a comment after them left aside.
fn quoted(s: &str) -> Option<&str> {
    let rest = s.strip_prefix('"')?;
    let mut escaped = false;
    for (i, c) in rest.char_indices() {
        match c {
            '\\' => escaped = !escaped,
            '"' if !escaped => return Some(&rest[..i]),
            _ => escaped = false,
        }
    }
    None
}

fn strip_markup(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = (depth - 1_i32).max(0),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `HALL PRINCIPAL` → `Hall principal`: the game's menu headings, as a title.
fn sentence_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut chars = lower.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

pub fn analyze(input: &Input) -> Result<Report, DriverError> {
    let file = input.files.iter().find(|f| !f.path.starts_with('@')).ok_or_else(|| DriverError::MissingFile("ProfileN.sav".into()))?;
    let save = save::decode(&file.bytes)?;
    let data: Data = serde_json::from_str(DATA).map_err(|e| DriverError::Corrupt(format!("data/hades.json : {e}")))?;
    let words = Words::read(input);
    let fr = input.is_french();
    let t = |f: &'static str, e: &'static str| if fr { f } else { e };
    let state = &save.state;
    let gs = state.get("GameState");
    let lines = state.get("TextLinesRecord");

    let mut report = Report::empty();
    if let Lua::Num(rev) = state.get("Revision") {
        report.game_version = Some(format!("{rev}"));
    }
    if save.god_mode {
        report.warnings.push(t("Mode Dieu activé sur ce profil.", "God Mode is on for this profile.").into());
    }

    // ——— Statistics
    let runs: Vec<&Lua> = gs.get("RunHistory").entries().map(|(_, r)| r).chain([state.get("CurrentRun")]).collect();
    let escapes = runs.iter().filter(|r| r.get("Cleared").truthy()).count();
    let deepest = runs.iter().map(|r| r.get("RunDepthCache").num() as u32).max().unwrap_or(0);
    let gained = gs.get("LifetimeResourcesGained");
    let have = gs.get("Resources");
    let codex = state.get("CodexStatus").entries().flat_map(|(_, cat)| cat.entries()).filter(|(_, e)| e.at(1).get("Unlocked").truthy()).count();
    let fish: u32 = gs.get("TotalCaughtFish").entries().map(|(_, n)| n.num() as u32).sum();
    let resource = |key: &str| format!("{} ({} {})", have.get(key).num() as u64, gained.get(key).num() as u64, t("gagnés en tout", "earned in all"));
    report.stats = vec![
        Stat::new("runs", t("Tentatives", "Runs"), save.runs.to_string()),
        Stat::new("escapes", t("Évasions", "Escapes"), escapes.to_string()),
        Stat::new("depth", t("Salle la plus profonde atteinte", "Deepest chamber reached"), deepest.to_string()),
        Stat::new("darkness", words.or("MetaPoints", t("Ténèbres", "Darkness")), resource("MetaPoints")),
        Stat::new("keys", words.or("LockKeys", t("Clés chthoniennes", "Chthonic Keys")), resource("LockKeys")),
        Stat::new("gems", words.or("Gems", t("Gemmes", "Gems")), resource("Gems")),
        Stat::new("nectar", words.or("GiftPoints", t("Nectar", "Nectar")), resource("GiftPoints")),
        Stat::new("diamonds", words.or("SuperGems", t("Diamants", "Diamonds")), resource("SuperGems")),
        Stat::new("titan", words.or("SuperLockKeys", t("Sang de Titan", "Titan Blood")), resource("SuperLockKeys")),
        Stat::new("ambrosia", words.or("SuperGiftPoints", t("Ambroisie", "Ambrosia")), resource("SuperGiftPoints")),
        Stat::new("codex", t("Entrées du Codex", "Codex entries"), codex.to_string()),
        Stat::new("fish", t("Poissons pêchés", "Fish caught"), fish.to_string()),
        Stat::new("cerberus", t("Caresses à Cerbère", "Cerberus pettings"), (gs.get("NumCerberusPettings").num() as u64).to_string()),
    ];

    // ——— Infernal Arms and their Aspects
    let realm_name = |id: &str| -> String { words.get(id).filter(|_| id != "Styx").unwrap_or_else(|| REALMS.iter().find(|r| r.0 == id).map(|r| if fr { r.1 } else { r.2 }).unwrap_or(id).to_string()) };
    let house = words.or("DeathArea", t("Maison d'Hadès", "House of Hades"));
    let mut weapons = Vec::new();
    let mut aspects = Vec::new();
    let aspects_open = gs.get("Flags").get("AspectsUnlocked").truthy();
    for w in &data.weapons {
        let name = words.or(&w.id, &w.id);
        let mut facts = Vec::new();
        let clears = gs.get("TimesClearedWeapon").get(&w.id).num() as u32;
        if clears > 0 {
            facts.push(format!("{clears} {}", t("évasion(s)", "escape(s)")));
        }
        let heat = gs.get("WeaponRecordsShrinePoints").get(&w.id).num() as u32;
        if heat > 0 {
            facts.push(format!("{} {heat}", t("Chaleur maximale", "Highest Heat")));
        }
        let time = gs.get("WeaponRecordsClearTime").get(&w.id).num();
        if time > 0.0 {
            facts.push(format!("{} {}", t("meilleur temps", "best time"), clear_time(time)));
        }
        let mut item = Item::check(&w.id, &name, gs.get("WeaponsUnlocked").get(&w.id).truthy()).location(&house);
        if !facts.is_empty() {
            item = item.hint(facts.join(" · "));
        }
        if let Some(link) = words.wiki(&w.id) {
            item = item.link(link);
        }
        weapons.push(item);

        for (index, a) in w.aspects.iter().enumerate() {
            let level = gs.get("WeaponUnlocks").get(&w.id).at(index as u32 + 1).num() as u32;
            let revealed = a.revealed_by.as_ref().is_none_or(|l| lines.get(l).truthy());
            // The base Aspects all bear Zagreus' name: whose arm it is tells them apart.
            let title = if a.starts_unlocked { format!("{} · {name}", words.or(&a.name, &a.name)) } else { words.or(&a.name, &a.name) };
            let mut item = Item::count(format!("aspect-{}", a.name), &title, level, a.max).location(&name);
            if level == 0 {
                item.status = if !aspects_open || !revealed { Status::Locked } else { Status::Todo };
            }
            item = item.hint(match (level, a.starts_unlocked, revealed) {
                (0, _, false) => t("Révélé par une conversation, puis à acheter au Miroir des Armes", "Revealed in a conversation, then bought at the arm").to_string(),
                (0, true, _) => t("Disponible dès le départ ; à renforcer avec du Sang de Titan", "Available from the start; strengthen it with Titan Blood").to_string(),
                (0, false, _) => t("À débloquer avec du Sang de Titan", "Unlock it with Titan Blood").to_string(),
                (l, ..) => format!("{} {l} / {}", t("Niveau", "Level"), a.max),
            });
            if a.revealed_by.is_some() {
                item = item.spoiler(Spoiler::Minor);
            }
            if let Some(link) = words.wiki(&a.name) {
                item = item.link(link);
            }
            aspects.push(item);
        }
    }

    // ——— Guardians and realms
    let kills = gs.get("EnemyKills");
    let guardians = GUARDIANS
        .iter()
        .map(|(key, realm)| {
            let n = kills.get(key).num() as u32;
            // Hades the guardian is the house's master.
            let name = words.get(key).or_else(|| words.get(&format!("NPC_{key}_01"))).unwrap_or_else(|| key.to_string());
            let mut item = Item::check(*key, name, n > 0).location(realm_name(realm));
            if n > 0 {
                item = item.hint(format!("{n} {}", t("fois vaincu", "times defeated")));
            }
            if let Some(link) = words.wiki(key) {
                item = item.link(link);
            }
            if *key == "Hades" { item.spoiler(Spoiler::Minor) } else { item }
        })
        .collect();
    let mut reached: Vec<&str> = Vec::new();
    for run in &runs {
        for (biome, v) in run.get("BiomesReached").entries() {
            if v.truthy() && !reached.contains(&biome) {
                reached.push(biome);
            }
        }
    }
    let realms = REALMS
        .iter()
        .map(|(id, ..)| Item::check(format!("realm-{id}"), realm_name(id), reached.contains(id) || (*id == "Surface" && escapes > 0)).location(realm_name(id)))
        .collect();

    // ——— The Fated List, whole and in the game's order
    let status_of = gs.get("QuestStatus");
    let prophecies: Vec<Item> = data
        .quests
        .iter()
        .map(|id| {
            let (status, hint) = match status_of.get(id).str() {
                Some("CashedOut") => (Status::Done, None),
                Some("Complete") => (Status::InProgress, Some(t("Accomplie : récompense à récupérer dans la Liste du destin", "Fulfilled: collect the reward in the Fated List"))),
                Some(_) => (Status::Todo, None),
                None => (Status::Locked, Some(t("Pas encore révélée par le jeu", "Not revealed by the game yet"))),
            };
            let name = words.get(id).or_else(|| {
                // `AchillesPatroclusReunion_B`: a later part of a prophecy the text names once.
                let (base, part) = id.rsplit_once('_')?;
                let first = words.get(&format!("{base}_A")).or_else(|| words.get(base))?;
                Some(format!("{first} ({part})"))
            });
            let mut item = Item::new(format!("quest-{id}"), name.unwrap_or_else(|| split_camel(id)), status);
            if let Some(h) = hint {
                item = item.hint(h);
            }
            if status == Status::Locked {
                item = item.spoiler(Spoiler::Minor);
            }
            item
        })
        .collect();

    // ——— Mirror of Night
    let ranks = gs.get("MetaUpgrades");
    let unlocked = gs.get("MetaUpgradesUnlocked");
    let mut mirror = Vec::new();
    for (row, pair) in data.mirror.iter().enumerate() {
        for (side, talent) in pair.iter().enumerate() {
            let rank = ranks.get(&talent.id).num() as u32;
            let name = words.get(&talent.id).or_else(|| words.get(&talent.icon)).unwrap_or_else(|| split_camel(&talent.icon));
            let mut item = Item::count(format!("mirror-{}", talent.id), name, rank, talent.max)
                .hint(format!("{} {rank} / {} · {} {}", t("Rang", "Rank"), talent.max, t("rangée", "row"), row + 1))
                .location(if side == 0 { t("Face noire", "Black side") } else { t("Face rouge", "Red side") });
            // The red side is opened with Chthonic Keys, talent by talent.
            if rank == 0 && side == 1 && !unlocked.get(&talent.id).truthy() {
                item.status = Status::Locked;
            }
            if let Some(link) = words.wiki(&talent.id).or_else(|| words.wiki(&talent.icon)) {
                item = item.link(link);
            }
            mirror.push(item);
        }
    }

    // ——— Relationships, keepsakes and companions
    let gift = gs.get("Gift");
    let hearts_of = |who: &str| gift.get(who).get("Value").num() as u32;
    let chambers = gs.get("KeepsakeChambers");
    let mut denizens = Vec::new();
    let mut gods = Vec::new();
    for (who, g) in &data.gifts {
        if matches!(gift.get(who), Lua::Nil) && !who.ends_with("Upgrade") {
            continue;
        }
        let n = hearts_of(who);
        let name = words.character(who);
        // Bouldy takes gifts forever: no last heart.
        let max = g.maximum.filter(|_| who != "NPC_Bouldy_01");
        let mut item = match max {
            Some(m) => Item::count(format!("bond-{who}"), &name, n, m).hint(format!("{n} / {m} {}", t("cœurs", "hearts"))),
            None => Item::new(format!("bond-{who}"), &name, if n > 0 { Status::InProgress } else { Status::Todo }).hint(format!("{n} {}", t("cœur(s)", "heart(s)"))),
        };
        if let (Some(locked), Some(m)) = (g.locked, max)
            && n >= locked
            && n < m
        {
            item.hint = Some(format!("{n} / {m} {} · {}", t("cœurs", "hearts"), t("les suivants s'ouvrent avec l'histoire et l'Ambroisie", "the next ones open with the story and Ambrosia")));
        }
        if let Some(link) = words.wiki(who) {
            item = item.link(link);
        }
        if who.ends_with("Upgrade") { gods.push(item) } else { denizens.push(item) }
    }
    // In the game's own order: the house, then Olympus.
    let order = |id: &str| data.gift_ordering.iter().position(|k| data.gifts.iter().any(|(w, g)| w == &id[5..] && g.keepsake.as_deref() == Some(k))).unwrap_or(usize::MAX);
    denizens.sort_by_key(|i| order(&i.id));
    gods.sort_by_key(|i| order(&i.id));

    let giver_of = |trait_name: &str| data.gifts.iter().find(|(_, g)| g.keepsake.as_deref() == Some(trait_name) || g.companion.as_deref() == Some(trait_name));
    let mut keepsakes = Vec::new();
    let mut companions = Vec::new();
    for k in &data.gift_ordering {
        let Some((who, g)) = giver_of(k) else { continue };
        let giver = words.character(who);
        let companion = g.companion.as_deref() == Some(k.as_str());
        let at = if companion { g.companion_at } else { g.keepsake_at }.unwrap_or(1);
        let owned = hearts_of(who) >= at;
        let title = words.or(k, k);
        let mut item = if companion {
            Item::check(k.clone(), &title, owned).hint(format!("{} {giver}, {} {at} {}", t("Offert par", "Given by"), t("à", "at"), t("cœurs", "hearts")))
        } else {
            // Ranks up with the chambers it is worn through.
            let worn = chambers.get(k).num() as u32;
            let rank = 1 + KEEPSAKE_RANKS.iter().filter(|r| worn >= **r).count() as u32;
            let mut item = Item::count(k.clone(), &title, if owned { worn } else { 0 }, KEEPSAKE_RANKS[1]);
            item.hint = Some(if owned {
                format!("{} {rank} / 3 · {worn} {} · {} {giver}", t("Rang", "Rank"), t("salles portées", "chambers worn"), t("offert par", "given by"))
            } else {
                let when = if at > 1 { format!("{} {at} {}", t("à", "at"), t("cœurs", "hearts")) } else { t("au premier cœur", "at the first heart").to_string() };
                format!("{} {giver}, {when}", t("Offert par", "Given by"))
            });
            if owned && worn == 0 {
                item.status = Status::InProgress;
            }
            item
        };
        if !owned {
            item.status = Status::Todo;
        }
        if let Some(link) = words.wiki(k) {
            item = item.link(link);
        }
        if companion { companions.push(item) } else { keepsakes.push(item) }
    }

    // ——— The House Contractor
    let added = gs.get("CosmeticsAdded");
    let slots = ["Critical", "CosmeticMainHall", "CosmeticNorthHall", "CosmeticLounge", "CosmeticBedroom", "Music"];
    let contractor = words.or("GhostAdminScreen_Title", t("Constructeur", "House Contractor"));
    let house_sections: Vec<Section> = slots
        .iter()
        .map(|slot| {
            let items = data
                .contractor
                .iter()
                .filter(|w| w.slot == *slot)
                .map(|w| {
                    // A song's name is its path run together: `/Music/MusicPlayer/X` → `MusicMusicPlayerX`.
                    let text_id = w.id.replace('/', "");
                    Item::check(format!("work-{}", w.id), words.or(&text_id, &split_camel(w.id.rsplit('/').next().unwrap_or(&w.id))), added.get(&w.id).truthy())
                })
                .collect();
            let heading = words.get(slot).map(|h| sentence_case(&h)).unwrap_or_else(|| slot.trim_start_matches("Cosmetic").to_string());
            Section::new(format!("contractor-{slot}"), format!("{contractor} · {heading}"), items).group(t("Maison d'Hadès", "House of Hades")).optional()
        })
        .collect();

    let arms = t("Armes", "Arms");
    let progress = t("Progression", "Progression");
    let bonds = t("Relations", "Relationships");
    report.sections = vec![
        Section::new("weapons", t("Armes infernales", "Infernal Arms"), weapons).group(arms),
        Section::new("aspects", t("Aspects des armes", "Weapon Aspects"), aspects).group(arms),
        Section::new("guardians", t("Gardiens", "Guardians"), guardians).group(progress),
        Section::new("realms", t("Royaumes atteints", "Realms reached"), realms).group(progress),
        Section::new("prophecies", words.or("QuestLogScreen_Title", t("Liste du destin", "Fated List")), prophecies).group(progress),
        Section::new("mirror", words.or("MetaUpgradeMenu", t("Miroir de la Nuit", "Mirror of Night")), mirror).group(progress),
        Section::new("denizens", t("Habitants des Enfers", "Underworld denizens"), denizens).group(bonds),
        Section::new("gods", t("Dieux de l'Olympe", "Olympians"), gods).group(bonds),
        Section::new("keepsakes", words.or("Keepsakes", t("Souvenirs", "Keepsakes")), keepsakes).group(bonds),
        Section::new("companions", t("Compagnons", "Companions"), companions).group(bonds),
    ];
    report.sections.extend(house_sections);
    Ok(report)
}

/// A clear time in seconds as the game shows it: `24:07.31`.
fn clear_time(seconds: f64) -> String {
    if seconds >= 3600.0 {
        return format_duration(seconds);
    }
    format!("{}:{:05.2}", (seconds / 60.0) as u32, seconds % 60.0)
}

/// `AresEarnKills` → `Ares Earn Kills`: a name when the game's text is missing.
fn split_camel(id: &str) -> String {
    let mut out = String::new();
    for (i, c) in id.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_games_names() {
        let text = br#"{
  Texts = [
    {
      Id = "SwordWeapon"
      DisplayName = "Lame du Styx"
    }
    {
      Id = "Health"
      DisplayName = "{!Icons.Health} Vie"
    }
    {
      Id = "NoName"
      InheritFrom = "Health"
    }
    {
      Id = "FirstStrikeMetaUpgrade"
      DisplayName = "{$Keywords.AlphaAttack}"
    }
    {
      Id = "Pride"
      DisplayName = "Gods' Pride" /* Paired with Gods' Legacy */
    }
  ]
}"#;
        let (names, aliases) = sjson_names(text);
        assert_eq!(names, vec![("SwordWeapon".into(), "Lame du Styx".into()), ("Health".into(), "Vie".into()), ("Pride".into(), "Gods' Pride".into())]);
        assert_eq!(aliases, vec![("FirstStrikeMetaUpgrade".into(), "AlphaAttack".into())]);
    }

    #[test]
    fn data_parses() {
        let data: Data = serde_json::from_str(DATA).unwrap();
        assert_eq!(data.weapons.len(), 6);
        assert!(data.weapons.iter().all(|w| w.aspects.len() == 4));
        assert_eq!(data.mirror.len(), 12);
        assert!(data.quests.len() > 50);
    }

    #[test]
    fn headings_and_times() {
        assert_eq!(sentence_case("HALL PRINCIPAL"), "Hall principal");
        assert_eq!(clear_time(1447.31), "24:07.31");
    }
}
