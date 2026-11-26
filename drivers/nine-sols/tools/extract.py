"""Builds data/nine-sols.json from the installed game: every collectible the
save records by id, with its name and description in English and French.

The game keeps its data in NineSols_Data/data.unity3d, a 2.7 GB bundle. What
is needed is unpacked from it block by block, without holding the bundle in
memory: resources.assets (the data), globalgamemanagers (the class and scene
names) and the scenes, read for where things lie (about 2 GB in the temporary
folder). Names come from the game's I2 Localization table, by term.

It writes data/nine-sols.json and, for the map (see worldmap.py),
data/areas.json (each area's name), data/zones.json (each zone's) and
assets/map-layout.json.

    python -m venv .venv && .venv/bin/pip install UnityPy lz4 TypeTreeGeneratorAPI
    .venv/bin/python tools/extract.py "path/to/Nine Sols" .
"""
import glob, json, lzma, os, re, struct, sys, tempfile

import lz4.block
import UnityPy

import addressables
import worldmap

# Columns of the I2 table the driver reports in.
LANGS = {"en": 0, "fr": 13}
GUID = re.compile(r"[0-9a-f]{32}")


def unbundle(path, wanted, out):
    """Writes the files of a UnityFS bundle named in `wanted` to `out`."""
    f = open(path, "rb")

    def cstr():
        b = b""
        while (c := f.read(1)) != b"\0":
            b += c
        return b

    cstr(); ver = struct.unpack(">I", f.read(4))[0]; cstr(); cstr()
    _, csize, usize, flags = struct.unpack(">QIII", f.read(20))
    if ver >= 7:
        f.seek((f.tell() + 15) // 16 * 16)
    if flags & 0x80:
        here = f.tell(); f.seek(-csize, 2); info = f.read(csize); f.seek(here)
    else:
        info = f.read(csize)
    if flags & 0x3F in (2, 3):
        info = lz4.block.decompress(info, uncompressed_size=usize)
    p = 16
    (n,) = struct.unpack_from(">I", info, p); p += 4
    blocks = []
    for _ in range(n):
        blocks.append(struct.unpack_from(">IIH", info, p)); p += 10
    (n,) = struct.unpack_from(">I", info, p); p += 4
    nodes = []
    for _ in range(n):
        off, size, _ = struct.unpack_from(">QQI", info, p); p += 20
        end = info.index(b"\0", p); name = info[p:end].decode(); p = end + 1
        if name in wanted:
            nodes.append((off, size, open(os.path.join(out, name), "wb")))
    if flags & 0x200:
        f.seek((f.tell() + 15) // 16 * 16)
    at = 0
    last = max(off + size for off, size, _ in nodes)
    for u, c, fl in blocks:
        if at >= last:
            break
        data = f.read(c)
        if not any(off < at + u and at < off + size for off, size, _ in nodes):
            at += u
            continue
        kind = fl & 0x3F
        if kind in (2, 3):
            data = lz4.block.decompress(data, uncompressed_size=u)
        elif kind == 1:
            data = lzma.LZMADecompressor(lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA1, "dict_size": 1 << 23}]).decompress(data[5:])
        for off, size, w in nodes:
            lo, hi = max(off, at), min(off + size, at + u)
            if lo < hi:
                w.write(data[lo - at:hi - at])
        at += u
    for *_, w in nodes:
        w.close()


def strings(raw):
    """The serialized strings of an object without its type tree, in order."""
    out, i = [], 0
    while i + 4 < len(raw):
        (n,) = struct.unpack_from("<i", raw, i)
        if 2 <= n < 400 and i + 4 + n <= len(raw):
            try:
                t = raw[i + 4:i + 4 + n].decode()
                if t.isprintable():
                    out.append(t)
                    i = (i + 4 + n + 3) // 4 * 4
                    continue
            except UnicodeDecodeError:
                pass
        i += 4
    return out


def i2_terms(raw):
    """I2 Localization's terms: each one's text in every column."""
    # After its name: three flags and an empty list, then the terms.
    p = (raw.find(b"I2Languages") + 11 + 3) // 4 * 4 + 12

    def i32():
        nonlocal p
        (v,) = struct.unpack_from("<i", raw, p); p += 4
        return v

    def s():
        nonlocal p
        n = i32(); v = raw[p:p + n].decode("utf-8", "replace"); p = (p + n + 3) // 4 * 4
        return v

    terms = {}
    for _ in range(i32()):
        term = s(); i32()
        langs = [s() for _ in range(i32())]
        flags = i32()
        p = (p + flags + 3) // 4 * 4
        for _ in range(i32()):
            s()
        terms[term] = langs
    return terms


# The lists the game itself counts from, when a class also holds content cut
# from the game: the jades of "Well Prepared", the skill tree of "The Warrior
# Within" (its name: "release version (for achievement statistics)"). What a
# shop sells is in the game too, even outside these lists.
OFFICIAL = {
    "jades": "0_JadeDataCollection",
    "skills": "AllSkillNodeDataCollection_正式版(成就統計用)",
}
# What refers to a tracked thing without being where it lies: the hand-over
# flags, the shops, the flags of the whole game.
RELAY_SKIP = {"ReceiveItemData", "MerchandiseData", "GameFlagCollection", "ItemDataCollection"}
# The zones without a map chip, named by the map's name of one of their areas
# (a scene the map draws, or the text's own key).
ZONE_TERMS = {
    "A0": "MinimapTitle/A0_S1_intro_morning",
    "A1": "MinimapTitle/Elevator_A1_S2",
    "AG": "AG_S2_YiBase",
}
# What is learnt rather than found: not on the map.
UNPLACED = {"skills"}
# Shops by the prefix of their merchandise's name.
SELLERS = {"蚩尤": "chiyou"}

# What the driver reports: each kind of data, the flag that says it is done,
# and the inventory type when one class holds several kinds.
SECTIONS = [
    ("seals", "ItemData", "Inventory/TypeTitle_Authority", "acquired"),
    ("jades", "JadeData", None, "acquired"),
    ("skills", "SkillNodeData", None, "acquired"),
    ("arrows", "PlayerWeaponData", "Inventory/TypeTitle_Equipment", "acquired"),
    ("computing", "PlayerAbilityData", "Inventory/TypeTitle_Equipment", "acquired"),
    ("tools", "ItemData", "Inventory/TypeTitle_Equipment", "acquired"),
    ("upgrades", "ItemData", "Inventory/TypeTitle_LevelUp", "acquired"),
    ("key-items", "ItemData", "Inventory/TypeTitle_Important", "acquired"),
    ("relics", "ItemData", "Inventory/TypeTitle_Relic", "acquired"),
    ("poisons", "ItemData", "Inventory/TypeTitle_Toxin", "acquired"),
    ("map-chips", "ItemData", "Inventory/TypeTitle_MapChip", "acquired"),
    ("nodes", "TeleportPointData", None, "unlocked"),
    # The database ("Collected Data"), in its two tabs.
    ("characters", "EncyclopediaItem", "Pedia/Type_Title_Character", "unlocked"),
    ("info", "EncyclopediaItem", "Pedia/Type_Title_Object", "unlocked"),
]

# The lists the game shows each kind in, in its order: a thing is listed in
# the first of them that holds it, by its rank there.
ORDER = [
    "0_JadeDataCollection",
    "AllSkillNodeDataCollection_正式版(成就統計用)",
    "(Arrow) PlayerWeaponDataCollection",
    "0_Inventory_Colliection_ImportantObjs",
    "1_Inventory_Colliection_Relics",
    "2_Inventory_Colliection_Materials",
    "3_Inventory_Colliection_Equipments",
    "Encyclopedia_Character",
    "Encyclopedia_Obj",
    "0_AllTeleportPoints",
]


def referrers(behaviours):
    """For each object, the objects that point to it (a PPtr in their data)."""
    out = {}
    for pid, (_, _, raw, _) in behaviours.items():
        for i in range(0, len(raw) - 12, 4):
            file_id, target = struct.unpack_from("<iq", raw, i)
            if file_id == 0 and target in behaviours and target != pid:
                out.setdefault(target, set()).add(pid)
    return out


def main(game, driver):
    data = os.path.join(game, "NineSols_Data")
    with tempfile.TemporaryDirectory() as tmp:
        # The data, the class names, the list of scenes, and the scenes.
        unbundle(os.path.join(data, "data.unity3d"), {"resources.assets", "globalgamemanagers.assets", "globalgamemanagers"} | {f"level{n}" for n in range(1000)}, tmp)
        env = UnityPy.load(os.path.join(tmp, "resources.assets"), os.path.join(tmp, "globalgamemanagers.assets"))
        res = next(f for n, f in env.files.items() if n.endswith("resources.assets"))
        externals = [os.path.basename(e.path) for e in res.externals]
        classes = {}
        for o in env.objects:
            if o.type.name == "MonoScript":
                try:
                    classes[(o.assets_file.name, o.path_id)] = o.read().m_ClassName
                except Exception:
                    pass
        objects, terms, behaviours = [], {}, {}
        for pid, o in res.objects.items():
            if o.type.name != "MonoBehaviour":
                continue
            raw = o.get_raw_data()
            # m_GameObject, m_Enabled, then m_Script: which class this is.
            file_id, path_id = struct.unpack_from("<iq", raw, 16)
            cls = classes.get((res.name if file_id == 0 else externals[file_id - 1], path_id))
            if cls == "LanguageSourceAsset":
                terms = i2_terms(raw)
                continue
            st = strings(raw)
            behaviours[pid] = (cls, st[0] if st else "", raw, st[1] if len(st) > 1 else "")
            if cls in {c for _, c, _, _ in SECTIONS}:
                objects.append((pid, cls, st))
        holders = referrers(behaviours)

        # The map: the game's world map panel.
        game_types = worldmap.Game(os.path.join(data, "Managed"), unity_version(os.path.join(tmp, "globalgamemanagers")))
        canvas, scenes, area_terms = worldmap.layout(res, behaviours, game_types)
        names = scene_names(os.path.join(tmp, "globalgamemanagers"))
        levels = sorted((int(f[5:]), os.path.join(tmp, f)) for f in os.listdir(tmp) if re.fullmatch(r"level\d+", f))
        wanted = {pid for pid, _, _ in objects}
        # Loot tables and the like: objects that hold one or two tracked
        # things, not lists of them all.
        relays = {}
        for pid in wanted:
            for h in holders.get(pid, ()):
                relays.setdefault(h, set()).add(pid)
        relays = {h: things for h, things in relays.items() if len(things) <= 2 and h not in wanted and behaviours[h][0] not in RELAY_SKIP}
        hidden = set(names) - set(scenes)
        found, doors = worldmap.scan(levels, names, wanted, relays, hidden)
        found.update(worldmap.nodes(res, behaviours, game_types))

        # Each thing's picture: an Addressables reference, named by the catalog.
        catalog = addressables.assets(os.path.join(data, "StreamingAssets", "aa", "catalog.json"))

        # The rank of each thing in the lists the game shows it in.
        rank = {}
        for pid, (cls, name, raw, _) in behaviours.items():
            if name in ORDER:
                listed = []
                for i in range(28, len(raw) - 12, 4):
                    fid, target = struct.unpack_from("<iq", raw, i)
                    if fid == 0 and target in behaviours and target != pid and target not in listed:
                        listed.append(target)
                for n, target in enumerate(listed):
                    rank.setdefault(target, (ORDER.index(name), n))

        out = {}
        for sid, cls, kind, field in SECTIONS:
            entries = []
            for pid, c, st in objects:
                if c != cls or len(st) < 2 or not GUID.fullmatch(st[1]):
                    continue
                held = {behaviours[h][:2] for h in holders.get(pid, ())}
                # Cut content: neither in the game's own count nor sold anywhere
                # (the skills bought from a shop are outside the achievement's).
                sold = any(c == "MerchandiseData" for c, _ in held)
                if sid in OFFICIAL and OFFICIAL[sid] not in {n for _, n in held} and not sold:
                    continue
                own = [t for t in st if t in terms]
                if kind and kind not in own:
                    continue
                title = next((t for t in own if "Type" not in t and "Desc" not in t and terms[t][0].strip()), None)
                if not title:
                    continue  # cut content: no name in the game's own text
                desc = next((t for t in own if "Desc" in t), None)
                entry = {"key": st[1] + cls, "field": field, "order": (rank.get(pid, (len(ORDER), 0)), st[0])}
                seller = next((v for c, n in held if c == "MerchandiseData" for k, v in SELLERS.items() if k in n), None)
                if seller:
                    entry["seller"] = seller
                # Handing it to someone (each poison to Shennong) is a flag of its
                # own, kept once the item itself is gone from the inventory.
                given = sorted(behaviours[h][3] + "ReceiveItemData" for h in holders.get(pid, ()) if behaviours[h][0] == "ReceiveItemData" and GUID.fullmatch(behaviours[h][3]))
                if given:
                    entry["given"] = given
                # Where it lies: skills are learnt, not found.
                spots = worldmap.places(found.get(pid, []), scenes, doors) if sid not in UNPLACED else []
                if spots:
                    entry["places"] = spots
                ref = game_types.read(res.objects[pid], cls).get("spriteRef", {}).get("assetReference", {}).get("m_AssetGUID", "")
                if ref in catalog:
                    entry["image"] = addressables.sprite_name(catalog[ref])
                for lang, col in LANGS.items():
                    entry[lang] = terms[title][col].strip() or terms[title][0].strip()
                    if desc and terms[desc][col].strip():
                        entry[f"{lang}_details"] = terms[desc][col].strip()
                entries.append(entry)
            entries.sort(key=lambda e: e["order"])
            for e in entries:
                del e["order"]
            out[sid] = entries
        # Each area of the map, named as the game names it.
        titles = {}
        for scene, key in area_terms.items():
            term = terms.get(key)
            if term:
                titles[scene] = {lang: (term[col].strip() or term[0].strip()) for lang, col in LANGS.items()}
        # The zones of New Kunlun (A2…), named as their map chips name them
        # ("Power Reservoir Chip"), or as the map names their first area.
        zones = {}
        for chip in out["map-chips"]:
            if chip.get("places"):
                zones[worldmap.region(chip["places"][0]["scene"])] = {lang: strip_chip(chip[lang], lang) for lang in LANGS}
        for code, term in ZONE_TERMS.items():
            term = area_terms.get(term, term)
            if code not in zones and term in terms:
                zones[code] = {lang: terms[term][col].strip() or terms[term][0].strip() for lang, col in LANGS.items()}
    write(os.path.join(driver, "data", "nine-sols.json"), out)
    write(os.path.join(driver, "data", "zones.json"), zones)
    write(os.path.join(driver, "data", "areas.json"), titles)
    write(os.path.join(driver, "assets", "map-layout.json"), canvas)


def strip_chip(name, lang):
    """A zone's name from its map chip's: "Power Reservoir Chip", "Puce du
    Réservoir d’Énergie"."""
    if lang == "fr":
        return re.sub(r"^Puce (du |de la |de l’|de l'|des |de )", "", name).strip()
    return re.sub(r"\s*Chip$", "", name).strip()


def unity_version(path):
    """The engine's version, as the player's files state it."""
    return re.search(rb"20\d\d\.\d+\.\d+f\d+", open(path, "rb").read(256)).group().decode()


def scene_names(path):
    """Each scene's name, by its number: `level73` is A10_S3_HistoryTomb_Right."""
    settings = next(o.read() for o in UnityPy.load(path).objects if o.type.name == "BuildSettings")
    return [os.path.basename(s).removesuffix(".unity") for s in settings.scenes]


def write(path, value):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=1)
        f.write("\n")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
