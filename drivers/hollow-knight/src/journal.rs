//! The Hunter's Journal: every creature the Hunter asks the Knight to study.
//!
//! The save keeps, for each entry, `killedX` (met and killed once) and
//! `killsX` (kills still needed before the Hunter's notes unlock; 0 when
//! complete). Names, descriptions and notes come from the game's own language
//! sheet when the host could read it from the installed game, portraits from
//! its sprites.

use hundo_sdk::{Counter, Item, Spoiler, Status};

use crate::Analysis;
use crate::spawns::{self, Change};

/// One entry, identifiers only, taken from the game's files (1.5): its
/// journal entry (save key, text key, portrait sprite) and the map areas of
/// the scenes whose enemies count towards it (their death effects name the
/// entry), the busiest first. Bosses the game spawns by script have their
/// area set by hand.
struct E {
    key: &'static str,
    text: &'static str,
    image: &'static str,
    /// The kills that unlock the Hunter's notes, as the game sets them for a
    /// new save (`killsX`, counted down); 0 where it gives none (Tiktik).
    kills: u32,
    dlc: Option<&'static str>,
    areas: &'static [&'static str],
}

/// In the game's order.
const ENTRIES: &[E] = &[
    E { key: "Crawler", text: "CRAWLER", image: "bestiary_crawler_f", kills: 0, dlc: None, areas: &["White Palace", "Crossroads", "Cliffs", "Waterways", "Resting_Grounds", "Green_Path", "Town_Tutorial"] },
    E { key: "Buzzer", text: "BUZZER", image: "bestiary_buzzer_f", kills: 45, dlc: None, areas: &["City of Tears", "Crossroads", "Cliffs", "Resting_Grounds", "Green_Path", "Town_Tutorial"] },
    E { key: "Bouncer", text: "BOUNCER", image: "bestiary_bouncer_f", kills: 25, dlc: None, areas: &["Crossroads"] },
    E { key: "Climber", text: "CLIMBER", image: "bestiary_climber_f", kills: 30, dlc: None, areas: &["Crossroads", "Cliffs", "Resting_Grounds", "Green_Path"] },
    E { key: "Hopper", text: "HOPPER", image: "bestiary_hopper_f", kills: 25, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "Worm", text: "WORM", image: "bestiary_armoured-worm_f", kills: 10, dlc: None, areas: &["Crossroads", "Deepnest"] },
    E { key: "Spitter", text: "SPITTER", image: "bestiary__0009_spitter", kills: 20, dlc: None, areas: &["Crossroads", "Ancient Basin"] },
    E { key: "Hatcher", text: "HATCHER", image: "bestiary_hatcher_f", kills: 15, dlc: None, areas: &["Crossroads"] },
    E { key: "Hatchling", text: "HATCHLING", image: "bestiary_hatcher_baby_f", kills: 30, dlc: None, areas: &["Crossroads"] },
    E { key: "ZombieRunner", text: "ZOM_RUNNER", image: "bestiary_bug_f", kills: 35, dlc: None, areas: &["Crossroads", "Deepnest", "Green_Path", "City of Tears"] },
    E { key: "ZombieHornhead", text: "ZOM_HORNHEAD", image: "bestiary_hornhead_f", kills: 35, dlc: None, areas: &["Crossroads", "City of Tears", "Deepnest", "Cliffs", "Green_Path", "Waterways", "Fungal Wastes"] },
    E { key: "ZombieLeaper", text: "ZOM_LEAPER", image: "bestiary_leaper_f", kills: 35, dlc: None, areas: &["Crossroads", "City of Tears", "Cliffs", "Green_Path"] },
    E { key: "ZombieBarger", text: "ZOM_BARGER", image: "bestiary_barger_f", kills: 35, dlc: None, areas: &["Crossroads", "City of Tears", "Cliffs", "Waterways", "Green_Path"] },
    E { key: "ZombieShield", text: "ZOM_SHIELD", image: "bestiary_shell-shielder_f", kills: 10, dlc: None, areas: &["Crossroads"] },
    E { key: "ZombieGuard", text: "ZOM_GUARD", image: "bestiary_crossroads-guard_f", kills: 6, dlc: None, areas: &["Crossroads"] },
    E { key: "BigBuzzer", text: "BIGBUZZER", image: "bestiary_big-buzzer_f", kills: 2, dlc: None, areas: &["Green_Path", "Kingdoms_Edge"] },
    E { key: "BigFly", text: "BIGFLY", image: "bestiary_big-bouncer_f", kills: 3, dlc: None, areas: &["Crossroads"] },
    E { key: "Mawlek", text: "MAWLEK", image: "bestiary_mawlek_f", kills: 1, dlc: None, areas: &["Crossroads"] },
    E { key: "FalseKnight", text: "FALSEKNIGHT", image: "bestiary_false-knight_f", kills: 1, dlc: None, areas: &["Crossroads"] },
    E { key: "Roller", text: "ROLLER", image: "bestiary_roller_f", kills: 20, dlc: None, areas: &[] },
    E { key: "Blocker", text: "BLOCKER", image: "bestiary_blocker_f", kills: 1, dlc: None, areas: &["Crossroads", "Cliffs"] },
    E { key: "PrayerSlug", text: "PRAYER_SLUG", image: "bestiary_prayer_slug_f", kills: 2, dlc: None, areas: &["Crossroads"] },
    E { key: "MenderBug", text: "MENDERBUG", image: "bestiary_menderbug_f", kills: 1, dlc: None, areas: &["Crossroads"] },
    E { key: "MossmanRunner", text: "MOSSMAN_RUNNER", image: "bestiary_mossman_runner_f", kills: 25, dlc: None, areas: &["Green_Path"] },
    E { key: "MossmanShaker", text: "MOSSMAN_SHAKER", image: "bestiary_mossman_shaker_f", kills: 25, dlc: None, areas: &["Green_Path"] },
    E { key: "Mosquito", text: "MOSQUITO", image: "bestiary_mosquito_f", kills: 25, dlc: None, areas: &["Green_Path"] },
    E { key: "BlobFlyer", text: "BLOB_FLYER", image: "bestiary_blob_fly_f", kills: 20, dlc: None, areas: &["Green_Path"] },
    E { key: "FungifiedZombie", text: "FUNG_ZOMBIE", image: "bestiary_fungus_zombie_02_f", kills: 10, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "PlantShooter", text: "PLANT_SHOOTER", image: "bestiary_spit_turret_f", kills: 15, dlc: None, areas: &["Green_Path"] },
    E { key: "MossCharger", text: "MOSS_CHARGER", image: "bestiary_moss_charger_f", kills: 15, dlc: None, areas: &["Green_Path"] },
    E { key: "MegaMossCharger", text: "MEGA_MOSS_CHARGER", image: "bestiary_mega_moss_charger_f", kills: 1, dlc: None, areas: &["Green_Path"] },
    E { key: "SnapperTrap", text: "SNAPPER_TRAP", image: "bestiary_snap_turret_f", kills: 15, dlc: None, areas: &["Green_Path", "Queens_Gardens"] },
    E { key: "MossKnight", text: "MOSS_KNIGHT", image: "bestiary_moss_knight_f", kills: 8, dlc: None, areas: &["Green_Path"] },
    E { key: "GrassHopper", text: "GRASS_HOPPER", image: "bestiary__0029_crazy-hopper", kills: 15, dlc: None, areas: &["Queens_Gardens"] },
    E { key: "AcidFlyer", text: "ACID_FLYER", image: "bestiary__0039_acid-flyer", kills: 8, dlc: None, areas: &["Green_Path"] },
    E { key: "AcidWalker", text: "ACID_WALKER", image: "bestiary__0037_acid-walker", kills: 8, dlc: None, areas: &["Green_Path", "Queens_Gardens"] },
    E { key: "MossFlyer", text: "MOSS_FLYER", image: "bestiary__0017_green-flyer", kills: 25, dlc: None, areas: &["Queens_Gardens", "Fog_Canyon"] },
    E { key: "MossKnightFat", text: "MOSS_KNIGHT_FAT", image: "bestiary_moss_knight_fat_f", kills: 10, dlc: None, areas: &["Queens_Gardens"] },
    E { key: "MossWalker", text: "MOSS_WALKER", image: "bestiary__0041_moss-crawler", kills: 30, dlc: None, areas: &["Green_Path", "Queens_Gardens"] },
    E { key: "InfectedKnight", text: "INFECTED_KNIGHT", image: "bestiary_infected_knight_f", kills: 1, dlc: None, areas: &["Ancient Basin"] },
    E { key: "LazyFlyer", text: "LAZY_FLYER", image: "bestiary_fung_lazy_flyer_f", kills: 1, dlc: None, areas: &["Green_Path", "Queens_Gardens"] },
    E { key: "ZapBug", text: "ZAP_BUG", image: "bestiary_zap_bugs_f", kills: 1, dlc: None, areas: &["Fog_Canyon"] },
    E { key: "Jellyfish", text: "JELLYFISH", image: "bestiary_jellyfish_f", kills: 12, dlc: None, areas: &["Fog_Canyon"] },
    E { key: "JellyCrawler", text: "JELLY_CRAWLER", image: "bestiary__0025_jelly-mini", kills: 20, dlc: None, areas: &["Fog_Canyon"] },
    E { key: "MegaJellyfish", text: "MEGA_JELLYFISH", image: "bestiary__0073_giant-jellyfish", kills: 1, dlc: None, areas: &["Godhome"] },
    E { key: "FungoonBaby", text: "FUNGOON_BABY", image: "bestiary__0033_fungoon-mini", kills: 30, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "MushroomTurret", text: "MUSH_TURRET", image: "bestiary_mushroom_turret_f", kills: 20, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "Mantis", text: "MANTIS", image: "bestiary_mantis_f", kills: 25, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "MushroomRoller", text: "MUSH_ROLLER", image: "bestiary_mushroom_roller_f", kills: 20, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "MushroomBrawler", text: "MUSH_BRAWLER", image: "bestiary_mushroom_brawler_f", kills: 8, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "MushroomBaby", text: "MUSH_BABY", image: "bestiary_mushroom_mini_f", kills: 20, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "MantisFlyerChild", text: "MANTIS_FLYER_CHILD", image: "bestiary_mantis_flyer_f", kills: 25, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "FungusFlyer", text: "FUNGOON", image: "bestiary__0031_fungoon", kills: 20, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "FungCrawler", text: "FUNG_CRAWLER", image: "bestiary__0015_fung-crawler", kills: 15, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "MantisLord", text: "MANTIS_LORD", image: "bestiary_mantis_lords_f", kills: 1, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "BlackKnight", text: "BLACK_KNIGHT", image: "bestiary_black_knight_f", kills: 10, dlc: None, areas: &["City of Tears"] },
    E { key: "ElectricMage", text: "ELEC_MAGE", image: "bestiary_electric_mage_f", kills: 6, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "Mage", text: "MAGE", image: "bestiary_mage_f", kills: 20, dlc: None, areas: &["City of Tears", "Kingdoms_Edge"] },
    E { key: "MageKnight", text: "MAGE_KNIGHT", image: "bestiary_mage_knight_f", kills: 2, dlc: None, areas: &["City of Tears", "Kingdoms_Edge"] },
    E { key: "RoyalDandy", text: "ZOM_ROYAL_1", image: "bestiary_royal_zombie_point_f", kills: 25, dlc: None, areas: &["City of Tears"] },
    E { key: "RoyalCoward", text: "ZOM_ROYAL_2", image: "bestiary_royal_zombie_flat_f", kills: 25, dlc: None, areas: &["City of Tears"] },
    E { key: "RoyalPlumper", text: "ZOM_ROYAL_FAT", image: "bestiary_royal_zombie_fat_f", kills: 25, dlc: None, areas: &["City of Tears"] },
    E { key: "FlyingSentrySword", text: "SENTRY_FLYING", image: "bestiary_ruins_zombie_flier_f", kills: 30, dlc: None, areas: &["City of Tears", "Waterways", "Resting_Grounds"] },
    E { key: "FlyingSentryJavelin", text: "SENTRY_FLYING_JAVELIN", image: "bestiary_ruins_zombie_lance_f", kills: 25, dlc: None, areas: &["City of Tears", "Waterways"] },
    E { key: "Sentry", text: "SENTRY_1", image: "bestiary_ruins_zombie_f", kills: 25, dlc: None, areas: &["City of Tears", "Waterways", "Resting_Grounds"] },
    E { key: "SentryFat", text: "SENTRY_FAT", image: "bestiary_ruins_zombie_big_f", kills: 20, dlc: None, areas: &["City of Tears", "Waterways"] },
    E { key: "MageBlob", text: "MAGEBLOB", image: "bestiary__0023_mage-blob", kills: 25, dlc: None, areas: &["City of Tears", "Kingdoms_Edge"] },
    E { key: "GreatShieldZombie", text: "GREAT_SHIELD_ZOMBIE", image: "bestiary__ruin_shield_guard_f", kills: 10, dlc: None, areas: &["City of Tears", "Resting_Grounds"] },
    E { key: "JarCollector", text: "JAR_COLLECTOR", image: "bestiary__0069_collector", kills: 1, dlc: None, areas: &["City of Tears"] },
    E { key: "MageBalloon", text: "MAGE_BALLOON", image: "bestiary__0021_mage-balloon", kills: 15, dlc: None, areas: &["City of Tears", "Kingdoms_Edge"] },
    E { key: "MageLord", text: "MAGE_LORD", image: "bestiary_mage_lord_f", kills: 1, dlc: None, areas: &["City of Tears"] },
    E { key: "GorgeousHusk", text: "GORGEOUS_HUSK", image: "bestiary_gold_royal_f", kills: 1, dlc: None, areas: &["City of Tears"] },
    E { key: "FlipHopper", text: "FLIP_HOPPER", image: "bestiary_flipper_f", kills: 20, dlc: None, areas: &["Waterways"] },
    E { key: "Flukeman", text: "FLUKEMAN", image: "bestiary_flukeman_f", kills: 20, dlc: None, areas: &["Waterways"] },
    E { key: "Inflater", text: "INFLATOR", image: "bestiary_inflater_f", kills: 20, dlc: None, areas: &["Waterways"] },
    E { key: "Flukefly", text: "FLUKEFLY", image: "bestiary__0055_fluke-fairy", kills: 15, dlc: None, areas: &["Waterways"] },
    E { key: "FlukeMother", text: "FLUKE_MOTHER", image: "bestiary__0057_fluke-mother", kills: 1, dlc: None, areas: &["Waterways"] },
    E { key: "DungDefender", text: "DUNG_DEFENDER", image: "bestiary__0005_dung-defender", kills: 1, dlc: None, areas: &["Waterways"] },
    E { key: "CrystalCrawler", text: "CRYS_CRAWLER", image: "bestiary_mines_goomba_f", kills: 15, dlc: None, areas: &["Crystal Peak", "Crossroads"] },
    E { key: "CrystalFlyer", text: "CRYS_FLYER", image: "bestiary_mines_flier_f", kills: 20, dlc: None, areas: &["Crystal Peak"] },
    E { key: "LaserBug", text: "CRYS_LAZER_BUG", image: "bestiary_mines_climber_f", kills: 15, dlc: None, areas: &["Crystal Peak"] },
    E { key: "BeamMiner", text: "ZOM_BEAM_MINER", image: "bestiary_zombie_miner_crystal_f", kills: 15, dlc: None, areas: &["Crystal Peak"] },
    E { key: "ZombieMiner", text: "ZOM_MINER", image: "bestiary_zombie_miner_f", kills: 20, dlc: None, areas: &["Crystal Peak", "Crossroads"] },
    E { key: "MegaBeamMiner", text: "MEGA_BEAM_MINER", image: "bestiary__0065_mega-beam-miner", kills: 2, dlc: None, areas: &["Crystal Peak"] },
    E { key: "MinesCrawler", text: "MINES_CRAWLER", image: "bestiary__0019_mines-crawler", kills: 15, dlc: None, areas: &["Crystal Peak"] },
    E { key: "AngryBuzzer", text: "ANGRY_BUZZER", image: "bestiary_infected_buzzer_f", kills: 15, dlc: None, areas: &["Crossroads"] },
    E { key: "BurstingBouncer", text: "BURSTING_BOUNCER", image: "bestiary_infected_fly_f", kills: 15, dlc: None, areas: &["Crossroads"] },
    E { key: "BurstingZombie", text: "ZOM_BURSTING", image: "bestiary_infected_husk1_f", kills: 15, dlc: None, areas: &["Crossroads"] },
    E { key: "SpittingZombie", text: "ZOM_SPITTING", image: "bestiary_infected_husk2_f", kills: 15, dlc: None, areas: &["Crossroads"] },
    E { key: "BabyCentipede", text: "BABY_CENTIPEDE", image: "bestiary_centipede_f", kills: 35, dlc: None, areas: &["Deepnest"] },
    E { key: "BigCentipede", text: "BIG_CENTIPEDE", image: "bestiary_big_centipede_f", kills: 10, dlc: None, areas: &["Deepnest"] },
    E { key: "CentipedeHatcher", text: "CENTIPEDE_HATCHER", image: "bestiary_centipede_hatcher_f", kills: 15, dlc: None, areas: &["Deepnest"] },
    E { key: "LesserMawlek", text: "LESSER_MAWLEK", image: "bestiary_mini_mawlek_f", kills: 10, dlc: None, areas: &["Ancient Basin"] },
    E { key: "SlashSpider", text: "SLASH_SPIDER", image: "bestiary_spider_soldier_f", kills: 15, dlc: None, areas: &["Deepnest"] },
    E { key: "SpiderCorpse", text: "ZOM_SPIDER", image: "bestiary_zombie_spider_01_f", kills: 15, dlc: None, areas: &["Deepnest"] },
    E { key: "ShootSpider", text: "SPITTING_SPIDER", image: "bestiary__0051_spit-spider", kills: 20, dlc: None, areas: &["Deepnest"] },
    E { key: "MiniSpider", text: "MINI_SPIDER", image: "bestiary_tiny_spider_f", kills: 25, dlc: None, areas: &["Deepnest"] },
    E { key: "SpiderFlyer", text: "SPIDER_FLYER", image: "bestiary__0011_spider-flyer", kills: 20, dlc: None, areas: &["Deepnest"] },
    E { key: "MimicSpider", text: "MIMIC_SPIDER", image: "bestiary__0059_mimic-spider", kills: 1, dlc: None, areas: &["Deepnest"] },
    E { key: "BeeHatchling", text: "BEE_HATCHLING", image: "bestiary_bee_tiny", kills: 30, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "BeeStinger", text: "BEE", image: "bestiary_bee_f", kills: 15, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "BigBee", text: "BIG_BEE", image: "bestiary_bee_big_f", kills: 12, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "HiveKnight", text: "HIVE_KNIGHT", image: "bestiary_hive_knight_f", kills: 1, dlc: Some("lifeblood"), areas: &["Kingdoms_Edge"] },
    E { key: "BlowFly", text: "BIG_FAT_FLYER", image: "bestiary__0053_blow-fly", kills: 20, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "CeilingDropper", text: "CEILING_DROPPER", image: "bestiary_bat_dropper_f", kills: 15, dlc: None, areas: &["Waterways", "City of Tears", "Kingdoms_Edge", "Ancient Basin", "Resting_Grounds"] },
    E { key: "GiantHopper", text: "GIANT_HOPPER", image: "bestiary__0071_giant-hopper", kills: 10, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "GrubMimic", text: "GRUB_MIMIC", image: "bestiary_mimic_grub_f", kills: 5, dlc: None, areas: &["Deepnest", "Crystal Peak"] },
    E { key: "MawlekTurret", text: "MAWLEK_TURRET", image: "bestiary_mawlek_turret_f", kills: 10, dlc: None, areas: &["Ancient Basin"] },
    E { key: "OrangeScuttler", text: "PARASITE_BLOB", image: "bestiary__0013_orange-blob", kills: 20, dlc: None, areas: &[] },
    E { key: "HealthScuttler", text: "HEALTH_SCUTTLER", image: "bestiary_blue_blob_f", kills: 10, dlc: None, areas: &[] },
    E { key: "Pigeon", text: "PIGEON", image: "bestiary_fung_pigeon_flyer_f", kills: 15, dlc: None, areas: &["Green_Path", "Queens_Gardens"] },
    E { key: "ZombieHive", text: "ZOM_HIVE", image: "bestiary_zombie_hive_f", kills: 10, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "Hornet", text: "HORNET", image: "bestiary_hornet", kills: 2, dlc: None, areas: &["Town_Tutorial", "Kingdoms_Edge", "Green_Path"] },
    E { key: "AbyssCrawler", text: "ABYSS_CRAWLER", image: "bestiary__0047_abyss-crawler", kills: 20, dlc: None, areas: &["Ancient Basin"] },
    E { key: "SuperSpitter", text: "SUPER_SPITTER", image: "bestiary_super_spitter_f", kills: 25, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "Sibling", text: "SIBLING", image: "bestiary__0045_shade-sibling", kills: 25, dlc: None, areas: &["Ancient Basin"] },
    E { key: "PalaceFly", text: "PALACE_FLY", image: "bestiary__0043_white-palace-fly", kills: 10, dlc: None, areas: &["White Palace"] },
    E { key: "EggSac", text: "EGG_SAC", image: "bestiary_egg_sac", kills: 5, dlc: None, areas: &["Waterways", "Crystal Peak", "Resting_Grounds", "Deepnest", "Kingdoms_Edge", "Queens_Gardens"] },
    E { key: "Mummy", text: "MUMMY", image: "bestiary__0007_grave-zombie", kills: 10, dlc: None, areas: &["Resting_Grounds"] },
    E { key: "OrangeBalloon", text: "ORANGE_BALLOON", image: "bestiary_parasite_flyer_f", kills: 10, dlc: None, areas: &["Ancient Basin"] },
    E { key: "AbyssTendril", text: "ABYSS_TENDRIL", image: "bestiary_abyss_tendrils_f", kills: 10, dlc: None, areas: &["Ancient Basin"] },
    E { key: "HeavyMantis", text: "MANTIS_HEAVY", image: "bestiary_heavy_mantis_f", kills: 15, dlc: None, areas: &["Queens_Gardens"] },
    E { key: "TraitorLord", text: "TRAITOR_LORD", image: "bestiary__0061_traitor-lord", kills: 1, dlc: None, areas: &["Queens_Gardens"] },
    E { key: "MantisHeavyFlyer", text: "MANTIS_FLYER_ADULT", image: "bestiary_heavy_mantis_flyer_f", kills: 16, dlc: None, areas: &["Queens_Gardens"] },
    E { key: "GardenZombie", text: "ZOM_SPINE", image: "bestiary_fungus_zombie_03_f", kills: 20, dlc: None, areas: &["Queens_Gardens"] },
    E { key: "RoyalGuard", text: "ROYAL_GUARD", image: "bestiary_kingsguard_f", kills: 2, dlc: None, areas: &["White Palace"] },
    E { key: "WhiteRoyal", text: "WHITE_ROYAL", image: "bestiary_white_servant_f", kills: 10, dlc: None, areas: &["White Palace"] },
    E { key: "Oblobble", text: "OBLOBBLE", image: "bestiary__0063_giant-fat-bee", kills: 2, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "Zote", text: "ZOTE", image: "bestiary__0049_zote", kills: 1, dlc: None, areas: &["Godhome"] },
    E { key: "Blobble", text: "BLOBBLE", image: "bestiary__0035_blobble", kills: 15, dlc: None, areas: &[] },
    E { key: "ColMosquito", text: "COL_MOSQUITO", image: "bestiary_colosseum_armoured_mosquito_f", kills: 15, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "ColRoller", text: "COL_ROLLER", image: "bestiary_colosseum_armoured_roller_f", kills: 20, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "ColFlyingSentry", text: "COL_FLYINGSENTRY", image: "bestiary_colosseum_flying_gladiator_f", kills: 25, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "ColMiner", text: "COL_MINER", image: "bestiary_colosseum_miner", kills: 25, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "ColShield", text: "COL_SHIELD", image: "bestiary_colosseum_shield_gladiator_f", kills: 25, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "ColWorm", text: "COL_WORM", image: "bestiary_colosseum_worm_f", kills: 20, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "ColHopper", text: "COL_HOPPER", image: "bestiary_colosseum_hopper_f", kills: 15, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "LobsterLancer", text: "LOBSTER_LANCER", image: "bestiary__0067_lobster-lancer", kills: 1, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "GhostAladar", text: "GHOST_ALADAR", image: "bestiary_ghosts__0000_brain-slug", kills: 1, dlc: None, areas: &["Cliffs"] },
    E { key: "GhostXero", text: "GHOST_XERO", image: "bestiary_ghosts__0001_xero", kills: 1, dlc: None, areas: &["Resting_Grounds"] },
    E { key: "GhostHu", text: "GHOST_HU", image: "bestiary_ghosts__0002_elderhu", kills: 1, dlc: None, areas: &["Fungal Wastes"] },
    E { key: "GhostMarmu", text: "GHOST_MARMU", image: "bestiary_ghosts__0005_marmu", kills: 1, dlc: None, areas: &["Queens_Gardens"] },
    E { key: "GhostNoEyes", text: "GHOST_NOEYES", image: "bestiary_ghosts__0006_no-eyes", kills: 1, dlc: None, areas: &["Godhome"] },
    E { key: "GhostMarkoth", text: "GHOST_MARKOTH", image: "bestiary_ghosts__0004_markoth", kills: 1, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "GhostGalien", text: "GHOST_GALIEN", image: "bestiary_ghosts__0003_galien", kills: 1, dlc: None, areas: &["Deepnest"] },
    E { key: "WhiteDefender", text: "WHITE_DEFENDER", image: "bestiary_White_Defender_full", kills: 1, dlc: Some("hidden-dreams"), areas: &["Godhome"] },
    E { key: "GreyPrince", text: "GREY_PRINCE", image: "bestiary_mighty_zote_full", kills: 1, dlc: Some("hidden-dreams"), areas: &["Godhome"] },
    E { key: "ZotelingBalloon", text: "ZOTE_BALLOON", image: "bestiary_mighty_zote_balloon_full", kills: 1, dlc: Some("hidden-dreams"), areas: &[] },
    E { key: "ZotelingHopper", text: "ZOTE_HOPPER", image: "bestiary_mighty_zote_hopper_full", kills: 1, dlc: Some("hidden-dreams"), areas: &[] },
    E { key: "ZotelingBuzzer", text: "ZOTE_FLYER", image: "bestiary_mighty_zoteling_full", kills: 1, dlc: Some("hidden-dreams"), areas: &[] },
    E { key: "HollowKnight", text: "HOLLOW_KNIGHT", image: "bestiary__0003_hollow-knight", kills: 1, dlc: None, areas: &["Crossroads"] },
    E { key: "FinalBoss", text: "FINAL_BOSS", image: "bestiary__0001_radiant_moth", kills: 1, dlc: None, areas: &["Crossroads"] },
    E { key: "HunterMark", text: "HUNTERMARK", image: "bestiary_hunter_mark_f", kills: 1, dlc: None, areas: &[] },
    E { key: "FlameBearerSmall", text: "FLAMEBEARER_SMALL", image: "Bestiary_Grimmkin_little", kills: 3, dlc: Some("grimm-troupe"), areas: &[] },
    E { key: "FlameBearerMed", text: "FLAMEBEARER_MED", image: "Bestiary_Grimmkin_fat", kills: 4, dlc: Some("grimm-troupe"), areas: &[] },
    E { key: "FlameBearerLarge", text: "FLAMEBEARER_LARGE", image: "Bestiary_Grimmkin_tall", kills: 5, dlc: Some("grimm-troupe"), areas: &[] },
    E { key: "Grimm", text: "GRIMM", image: "Bestiary_Grimm_boss", kills: 1, dlc: Some("grimm-troupe"), areas: &["Town_Tutorial"] },
    E { key: "NightmareGrimm", text: "NIGHTMARE_GRIMM", image: "Bestiary_Grimm_Nightmare_boss", kills: 1, dlc: Some("grimm-troupe"), areas: &["Town_Tutorial"] },
    E { key: "BindingSeal", text: "KIN_SEAL", image: "vessel_seal", kills: 1, dlc: None, areas: &["Godhome"] },
    E { key: "FatFluke", text: "FAT_FLUKE", image: "bestiary_fat_fluke_full", kills: 8, dlc: None, areas: &["Waterways"] },
    E { key: "PaleLurker", text: "PALE_LURKER", image: "bestiary_gg__0009_pale_runaway", kills: 1, dlc: None, areas: &["Kingdoms_Edge"] },
    E { key: "NailBros", text: "NAILMASTERS", image: "bestiary_gg__0006_nailmasters", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
    E { key: "Paintmaster", text: "PAINTMASTER", image: "bestiary_gg__0007_paintmaster", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
    E { key: "Nailsage", text: "SAGE_SLY", image: "bestiary_gg__0005_sage", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
    E { key: "HollowKnightPrime", text: "PURE_VESSEL", image: "bestiary_gg__0008_HK_prime", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
    E { key: "GodseekerMask", text: "GODSEEKER_MASK", image: "bestiary_gg_mask", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
    E { key: "VoidIdol_1", text: "VOID_IDOL_1", image: "bestiary_gg_void_idol", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
    E { key: "VoidIdol_2", text: "VOID_IDOL_2", image: "bestiary_gg_void_idol_states_0001_2", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
    E { key: "VoidIdol_3", text: "VOID_IDOL_3", image: "bestiary_gg_void_idol_states_0000_3", kills: 1, dlc: Some("godmaster"), areas: &["Godhome"] },
];

/// What the Hunter asks for his mark, as the game counts it
/// (`PlayerData.CountJournalEntries`): every entry, but these, which add to
/// `journalEntriesTotal` only once met.
const OPTIONAL: &[&str] = &[
    "MenderBug", "Zote", "WhiteDefender", "GreyPrince", "ZotelingBuzzer", "ZotelingHopper", "ZotelingBalloon", "HollowKnight",
    "FinalBoss", "FlameBearerSmall", "FlameBearerMed", "FlameBearerLarge", "Grimm", "NightmareGrimm", "NailBros", "Paintmaster",
    "Nailsage", "HollowKnightPrime",
];
/// Entries the game never counts: Godhome's.
const UNCOUNTED: &[&str] = &["BindingSeal", "GodseekerMask", "VoidIdol_1", "VoidIdol_2", "VoidIdol_3"];
/// The mark itself: an entry of its own, the reward for the others.
const MARK: &str = "HunterMark";

/// Whether an entry is needed for the Hunter's Mark.
pub fn required(key: &str) -> bool {
    !OPTIONAL.contains(&key) && !UNCOUNTED.contains(&key)
}

impl Analysis<'_> {
    /// The journal in two: what the Hunter's Mark asks for, and the rest.
    pub(crate) fn journal(&self) -> (Vec<Item>, Vec<Item>) {
        let all = self.journal_entries();
        // The mark counts the required entries complete, notes included.
        let needed: Vec<&str> = ENTRIES.iter().map(|e| e.key).filter(|k| required(k) && *k != MARK).collect();
        let complete = needed
            .iter()
            .filter(|k| self.bool(&format!("killed{k}")) == Some(true) && self.int(&format!("kills{k}")) <= 0)
            .count() as u32;
        let (mut req, opt): (Vec<Item>, Vec<Item>) = all.into_iter().partition(|i| required(i.id.trim_start_matches("journal-")));
        if let Some(mark) = req.iter_mut().find(|i| i.id == format!("journal-{MARK}")) {
            mark.progress = Some(Counter { current: complete, total: needed.len() as u32 });
            if mark.status != Status::Done {
                mark.hint = Some(if self.fr {
                    format!("Le Chasseur la donne une fois les {} entrées obligatoires complètes, notes comprises", needed.len())
                } else {
                    format!("The Hunter gives it once the {} required entries are complete, notes included", needed.len())
                });
            }
        }
        (req, opt)
    }

    fn journal_entries(&self) -> Vec<Item> {
        ENTRIES
            .iter()
            .filter_map(|e| {
                let killed = self.bool(&format!("killed{}", e.key))?;
                let left = self.int(&format!("kills{}", e.key));
                let name = self.text(&format!("NAME_{}", e.text)).unwrap_or_else(|| spaced(e.key));
                // The three Void Idols share one name: the Hall of Gods' difficulty tells them apart.
                let name = match e.key {
                    "VoidIdol_1" => format!("{name} · {}", self.t("Synchronisé", "Attuned")),
                    "VoidIdol_2" => format!("{name} · {}", self.t("Transcendé", "Ascended")),
                    "VoidIdol_3" => format!("{name} · {}", self.t("Radiant", "Radiant")),
                    _ => name,
                };
                let status = match (killed, left) {
                    (true, n) if n <= 0 || !self.full_journal => Status::Done,
                    (true, _) => Status::InProgress,
                    _ => Status::Todo,
                };
                // The Crossroads' infection swaps their creatures: those gone
                // from there are no longer met there.
                let change = spawns::change(e.key, self.infected());
                let areas = e.areas.iter().filter(|a| !(change == Some(Change::LeftCrossroads) && **a == "Crossroads"));
                let mut item = Item::new(format!("journal-{}", e.key), name, status)
                    .image(format!("game:{}", e.image))
                    .areas(areas.map(|a| crate::game_area(a).unwrap_or(a)));
                // The kills counted towards the Hunter's notes, where it takes more than one.
                if e.kills > 1 {
                    let current = if killed { e.kills - (left.clamp(0, i64::from(e.kills)) as u32) } else { 0 };
                    item.progress = Some(Counter { current, total: e.kills });
                }
                let mut hints: Vec<String> = Vec::new();
                // The count says it, where there is one.
                if killed && left > 0 && e.kills <= 1 {
                    hints.push(if self.fr {
                        format!("Encore {left} à vaincre pour les notes du Chasseur")
                    } else {
                        format!("{left} more to defeat for the Hunter's notes")
                    });
                }
                let crossroads = self.zone("Forgotten Crossroads");
                match change {
                    Some(Change::NotYet) => hints.push(if self.fr {
                        format!("N'apparaît qu'une fois l'Infection répandue : {crossroads}")
                    } else {
                        format!("Only appears once the Infection has spread through the {crossroads}")
                    }),
                    Some(Change::Gone) => hints.push(if self.fr {
                        format!("N'apparaît plus depuis que l'Infection s'est répandue : {crossroads}")
                    } else {
                        format!("No longer appears since the Infection spread through the {crossroads}")
                    }),
                    Some(Change::LeftCrossroads) => hints.push(if self.fr {
                        format!("Plus là depuis l'Infection : {crossroads}")
                    } else {
                        format!("No longer in the {crossroads} since the Infection")
                    }),
                    None => {}
                }
                if UNCOUNTED.contains(&e.key) {
                    hints.push(self.t("Hors du décompte du journal", "Not counted by the journal").to_string());
                } else if OPTIONAL.contains(&e.key) {
                    hints.push(self.t("Facultative pour la Marque du Chasseur", "Not needed for the Hunter's Mark").to_string());
                }
                if !hints.is_empty() {
                    item = item.hint(hints.join(" · "));
                }
                // What the journal shows: the description once met, the
                // Hunter's notes once the entry is complete.
                let mut details: Vec<String> = Vec::new();
                if let Some(d) = self.text(&format!("DESC_{}", e.text)) {
                    details.push(d);
                }
                if status == Status::Done
                    && let Some(n) = self.text(&format!("NOTE_{}", e.text))
                {
                    details.push(format!("{} {n}", self.t("Notes du Chasseur :", "Hunter's notes:")));
                }
                if !details.is_empty() {
                    item = item.details(details.join("\n\n"));
                }
                // An entry not yet met is a creature the reader has not seen: the
                // game itself keeps it hidden.
                if !killed {
                    item = item.spoiler(Spoiler::Major);
                }
                Some(match e.dlc {
                    Some(d) => item.dlc(d),
                    None => item,
                })
            })
            .collect()
    }
}

/// A Journal entry's text key and portrait, by its save key.
pub fn entry(key: &str) -> Option<(&'static str, &'static str)> {
    ENTRIES.iter().find(|e| e.key == key).map(|e| (e.text, e.image))
}

/// The Journal entry of each boss item: its portrait and its official name.
pub const BOSSES: &[(&str, &str)] = &[
    ("killedBigFly", "BigFly"),
    ("falseKnightDefeated", "FalseKnight"),
    ("hornet1Defeated", "Hornet"),
    ("killedMawlek", "Mawlek"),
    ("defeatedDungDefender", "DungDefender"),
    ("mageLordDefeated", "MageLord"),
    ("defeatedMantisLords", "MantisLord"),
    ("killedMimicSpider", "MimicSpider"),
    ("killedInfectedKnight", "InfectedKnight"),
    ("collectorDefeated", "JarCollector"),
    ("defeatedMegaJelly", "MegaJellyfish"),
    ("hornetOutskirtsDefeated", "Hornet"),
    ("killedTraitorLord", "TraitorLord"),
    ("killedBlackKnight", "BlackKnight"),
    ("xeroDefeated", "GhostXero"),
    ("aladarSlugDefeated", "GhostAladar"),
    ("elderHuDefeated", "GhostHu"),
    ("mumCaterpillarDefeated", "GhostMarmu"),
    ("noEyesDefeated", "GhostNoEyes"),
    ("galienDefeated", "GhostGalien"),
    ("markothDefeated", "GhostMarkoth"),
    ("killedHiveKnight", "HiveKnight"),
    ("killedWhiteDefender", "WhiteDefender"),
    ("greyPrinceDefeated", "GreyPrince"),
    ("killedGrimm", "Grimm"),
    ("killedNightmareGrimm", "NightmareGrimm"),
    ("nightmareKing", "NightmareGrimm"),
    ("killedHollowKnight", "HollowKnight"),
    ("killedFinalBoss", "FinalBoss"),
];

/// `MossKnightFat` → `Moss Knight Fat`, when the game's names are not at hand.
fn spaced(key: &str) -> String {
    let mut out = String::new();
    for (i, c) in key.chars().enumerate() {
        if i > 0 && (c.is_ascii_uppercase() || c == '_') {
            out.push(' ');
        }
        if c != '_' {
            out.push(c);
        }
    }
    out
}
