#!/usr/bin/env python3
"""Builds data/hades.json from the game's own scripts (Content/Scripts/*.lua):
the lists Hundo checks a save against, by the game's identifiers only. The
names come from the reader's copy of the game when the module runs.

    python3 tools/extract.py "<Hades>/Content/Scripts" > data/hades.json
"""
import json
import re
import sys
from pathlib import Path


def block(text, start):
    """The `{ ... }` that opens at or after `start`, braces balanced, strings skipped."""
    i = text.index("{", start)
    depth, j, quote = 0, i, None
    while True:
        c = text[j]
        if quote:
            if c == "\\":
                j += 1
            elif c == quote:
                quote = None
        elif c in "\"'":
            quote = c
        elif c == "-" and text.startswith("--", j):
            j = text.index("\n", j)
            continue
        elif c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return text[i + 1 : j]
        j += 1


def entries(body):
    """The keyed tables of a table's body, one level down: (key, body)."""
    out, pos = [], 0
    pat = re.compile(r'(?m)^\t(?:\["([^"]+)"\]|([A-Za-z_][A-Za-z_0-9]*))\s*=\s*\n?\s*\{')
    while True:
        m = pat.search(body, pos)
        if not m:
            return out
        inner = block(body, m.end() - 1)
        out.append((m.group(1) or m.group(2), inner))
        pos = m.end() - 1 + len(inner) + 2


def strings(body):
    return re.findall(r'"([^"]+)"', re.sub(r"--[^\n]*", "", body))


def table(path, name):
    text = Path(path).read_text(encoding="utf-8-sig")
    m = re.search(rf"(?m)^{re.escape(name)}\s*=", text)
    return block(text, m.end())


def main(scripts):
    s = Path(scripts)

    weapons = []
    for weapon, body in entries(table(s / "WeaponUpgradeData.lua", "WeaponUpgradeData")):
        if weapon.startswith("Default"):
            continue
        aspects = []
        for aspect in re.findall(r"(?ms)^\t\t\{(.*?)^\t\t\}", body):
            trait = re.search(r'(?m)^\s*(?:TraitName|RequiredInvestmentTraitName)\s*=\s*"([^"]+)"', aspect).group(1)
            aspects.append({
                "trait": trait,
                "max": int(re.search(r"MaxUpgradeLevel\s*=\s*(\d+)", aspect).group(1)),
                "startsUnlocked": "StartsUnlocked = true" in aspect,
                # Revealed by Nyx's story, by this line of hers, before it can be bought.
                "revealedBy": next(iter(re.findall(r'RequiredTextLines\s*=\s*\{\s*"([^"]+)"', aspect)), None),
            })
        weapons.append({"id": weapon, "aspects": aspects})

    quests = strings(table(s / "QuestData.lua", "QuestOrderData"))

    gift_text = table(s / "GiftData.lua", "GiftData")
    gifts = {}
    templates = dict(entries(gift_text))
    for who, body in entries(gift_text):
        if who.startswith("Default") or who.endswith("Trait"):
            continue
        # What it does not say, it takes from the template it inherits from.
        parent = re.search(r'InheritFrom\s*=\s*\{\s*"([^"]+)"', body)
        base = templates.get(parent.group(1), "") if parent else ""
        maximum = re.search(r"Maximum\s*=\s*(\d+)", body) or re.search(r"Maximum\s*=\s*(\d+)", base)
        locked = re.search(r"Locked\s*=\s*(\d+)", body) or re.search(r"Locked\s*=\s*(\d+)", base)
        given = {int(n): g for n, g in re.findall(r'\[(\d+)\]\s*=\s*\{\s*Gift\s*=\s*"([^"]+)"', body)}
        companion = next(((n, g) for n, g in given.items() if g.endswith("AssistTrait")), None)
        # The keepsake: given at the first heart, or later (Hades' own).
        keepsake = next(((n, g) for n, g in sorted(given.items()) if not g.endswith("AssistTrait")), None)
        gifts[who] = {
            "maximum": int(maximum.group(1)) if maximum else None,
            "locked": int(locked.group(1)) if locked else None,
            "keepsake": keepsake[1] if keepsake else None,
            "keepsakeAt": keepsake[0] if keepsake else None,
            "companion": companion[1] if companion else None,
            "companionAt": companion[0] if companion else None,
        }
    ordering = strings(table(s / "GiftData.lua", "GiftOrdering"))

    meta = dict(entries(table(s / "MetaUpgradeData.lua", "MetaUpgradeData")))
    mirror = []
    for pair in re.findall(r"\{\s*\"([^\"]+)\"\s*,\s*\"([^\"]+)\"\s*\}", table(s / "MetaUpgradeData.lua", "MetaUpgradeOrder")):
        row = []
        for talent in pair:
            body = meta[talent]
            icon = re.search(r'Icon\s*=\s*"MirrorIcon_([^"]+)"', body).group(1)
            # Ranks: one per cost of its table, or a flat cost up to a maximum.
            costs = re.search(r"CostTable\s*=\s*\{([^}]*)\}", body)
            ranks = len([c for c in costs.group(1).split(",") if c.strip()]) if costs else int(re.search(r"MaxInvestment\s*=\s*(\d+)", body).group(1))
            row.append({"id": talent, "icon": icon, "max": ranks})
        mirror.append(row)

    items = entries(table(s / "ConditionalItemData.lua", "ConditionalItemData"))
    slots = {}
    contractor = []
    for name, body in items:
        slot = re.search(r'(?m)^\t\tSlot\s*=\s*"([^"]+)"', body)
        parent = re.search(r'InheritFrom\s*=\s*\{\s*"([^"]+)"', body)
        slot = slot.group(1) if slot else slots.get(parent.group(1)) if parent else None
        slots[name] = slot
        if "DebugOnly = true" in body or not slot:
            continue
        contractor.append({"id": name, "slot": slot})
    # The music player's songs, sold by the Contractor too.
    for name, body in entries(table(s / "AudioData.lua", "MusicPlayerTrackData")):
        if "DebugOnly = true" not in body:
            contractor.append({"id": name, "slot": "Music"})

    print(json.dumps({
        "source": "Hades' own scripts, identifiers only (tools/extract.py)",
        "weapons": weapons,
        "quests": quests,
        "gifts": gifts,
        "giftOrdering": ordering,
        "mirror": mirror,
        "contractor": contractor,
    }, indent=1, ensure_ascii=False))


main(sys.argv[1])
