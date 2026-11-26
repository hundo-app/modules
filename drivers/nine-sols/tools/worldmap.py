"""The map of New Kunlun, read from the installed game: the game's own world
map, and where each thing the driver tracks lies on it.

The world map is a UI panel (`CONTENT`): a block per scene, tinted by its
zone's colour (`ColorProvider`), the passages between them, and the zones'
symbols. Its images are walked as the game lays them out (anchored position,
scale, mirror) into `assets/map-layout.json`, which only says which of the
game's sprites goes where: Hundo draws them from the reader's copy, and
nothing drawn ships with the driver.

A thing's place is its spot in its scene, as fractions of the scene's bounds
(`GameLevelMapData`, `MapMinX`…`MapMaxY`) from the left and from the bottom,
found where the scene's own objects refer to it (the action that gives it,
the save point of a node); the driver puts it at that spot of the scene's
block, as the game puts its own icons.
"""
import glob, json, os, struct

import UnityPy
from TypeTreeGeneratorAPI import TypeTreeGenerator

# The panel holding the world map's blocks.
CONTENT = "(Shared Content For WorldMap And Teleport Map)Content"
# Hundo's canvas: the panel's units, with a margin.
MARGIN = 24
# What the panel shows of the player, the cursor and hidden hints: not drawn.
SKIP = ("Cursor", "IconContainer", "沒晶片標示", "Door", "backGroundImage", "[設定]")
# The image of a block that draws its scene.
BLOCK = "Map Image"
# Objects that refer to every item at once, not to where one lies.
NOISE = ("EffectCamera",)
# Scenes that are not part of the world: a test room that gives everything.
OFF_WORLD = ("VR_Challenge_Hub",)
# Objects that give the thing, by their name: where several objects refer to
# it, these are where it lies (the others only check for it).
GIVERS = ("Pick", "GetItem", "Get Item", "ItemGetUIShow", "取得", "拿到", "SavePoint")
class Game:
    """The game's scripts' fields, from its own assemblies."""

    def __init__(self, managed, unity):
        self.gen = TypeTreeGenerator(unity)
        for f in sorted(glob.glob(os.path.join(managed, "*.dll"))):
            try:
                self.gen.load_dll(open(f, "rb").read())
            except Exception:
                pass
        self.trees = {}

    def read(self, obj, cls):
        if cls not in self.trees:
            self.trees[cls] = json.loads(self.gen.get_nodes_as_json("Assembly-CSharp.dll", cls))
        return obj.read_typetree(self.trees[cls])


def transform(go, objects):
    for c in go.m_Components:
        o = objects.get(c.path_id)
        if o is not None and o.type.name in ("RectTransform", "Transform"):
            return o.read()
    return None


def layout(res, behaviours, game):
    """The world map panel, drawn as the game lays it out: each image, tinted
    by its colour and its zone's, at its place. The blocks are rooms, by
    their scene."""
    objects = res.objects
    content = next(o.read() for o in objects.values() if o.type.name == "GameObject" and o.read().m_Name == CONTENT)
    images, scenes, titles = [], {}, {}

    def component(go, cls):
        return next((c.path_id for c in go.m_Components if behaviours.get(c.path_id, ("",))[0] == cls), None)

    def walk(t, x, y, sx, sy, tint, scene):
        go = t.m_GameObject.read()
        if not go.m_IsActive or go.m_Name.startswith(SKIP):
            return
        ap = t.m_AnchoredPosition
        x, y = x + ap.x * sx, y + ap.y * sy
        sx, sy = sx * t.m_LocalScale.x, sy * t.m_LocalScale.y
        provider = component(go, "ColorProvider")
        if provider is not None:
            c = game.read(objects[provider], "ColorProvider")["color"]
            tint = (c["r"], c["g"], c["b"], c["a"])
        button = component(go, "MinimapAreaUIButton")
        if button is not None:
            data = game.read(objects[game.read(objects[button], "MinimapAreaUIButton")["mapData"]["m_PathID"]], "GameLevelMapData")
            scene = data["sceneID"]
            scenes[scene] = (data["MapMinX"], data["MapMinY"], data["MapMaxX"], data["MapMaxY"])
            titles[scene] = data["MinimapSceneName"]["mTerm"]
        image = component(go, "Image")
        # Stretched images take their parent's size: none on this panel draws.
        if image is not None and t.m_AnchorMin.x == t.m_AnchorMax.x and t.m_AnchorMin.y == t.m_AnchorMax.y:
            sprite, colour = ui_image(behaviours[image][2], objects)
            if sprite:
                w, h = t.m_SizeDelta.x * abs(sx), t.m_SizeDelta.y * abs(sy)
                cx, cy = x + (0.5 - t.m_Pivot.x) * w, y + (0.5 - t.m_Pivot.y) * h
                images.append({
                    "sprite": sprite,
                    "x": cx, "y": cy, "w": w, "h": h,
                    "mirror": sx < 0,
                    "tint": [round(a * b, 4) for a, b in zip(colour, tint)],
                    # The block's own picture: the scene's room on the map.
                    "scene": scene if go.m_Name == BLOCK else None,
                })
        for child in t.m_Children:
            walk(child.read(), x, y, sx, sy, tint, scene)

    for child in transform(content, objects).m_Children:
        walk(child.read(), 0.0, 0.0, 1.0, 1.0, (1.0, 1.0, 1.0, 1.0), None)
    x0 = min(i["x"] - i["w"] / 2 for i in images) - MARGIN
    y1 = max(i["y"] + i["h"] / 2 for i in images) + MARGIN
    width = max(i["x"] + i["w"] / 2 for i in images) + MARGIN - x0
    height = y1 - min(i["y"] - i["h"] / 2 for i in images) + MARGIN
    pieces = []
    for i in images:
        piece = {"sprite": i["sprite"], "rect": rnd([i["x"] - i["w"] / 2 - x0, y1 - (i["y"] + i["h"] / 2), i["w"], i["h"]])}
        if i["mirror"]:
            piece["crop"] = [1, 0, 0, 1]
        if i["tint"] != [1.0, 1.0, 1.0, 1.0]:
            piece["tint"] = i["tint"]
        if i["scene"]:
            piece.update({"scene": i["scene"], "room": True})
        pieces.append(piece)
    return {"width": round(width), "height": round(height), "pieces": pieces}, scenes, titles


def ui_image(raw, objects):
    """A UI image's sprite and colour, from its fields: after the header,
    `m_Material` then `m_Color`, and further on `m_Sprite`."""
    (n,) = struct.unpack_from("<i", raw, 28)
    o = 28 + 4 + (n + 3) // 4 * 4 + 12
    colour = struct.unpack_from("<4f", raw, o)
    for i in range(o + 16, len(raw) - 12, 4):
        fid, pid = struct.unpack_from("<iq", raw, i)
        if fid == 0 and pid in objects and objects[pid].type.name == "Sprite":
            return objects[pid].read().m_Name, colour
    return None, colour


def region(scene):
    """A scene's zone, by its name's first part: `A2_S1_…` is in A2."""
    return scene.split("_")[0]


def rnd(values, digits=1):
    return [round(v, digits) for v in values]


def nodes(res, behaviours, game):
    """Root nodes: each knows its scene and the spot it brings Yi to."""
    out = {}
    for pid, (cls, *_) in behaviours.items():
        if cls != "TeleportPointData":
            continue
        t = game.read(res.objects[pid], "TeleportPointData")
        if t["levelMapData"]["m_PathID"]:
            scene = game.read(res.objects[t["levelMapData"]["m_PathID"]], "GameLevelMapData")["sceneID"]
            out[pid] = [(scene, t["TeleportPosition"]["x"], t["TeleportPosition"]["y"], "SavePoint", False)]
    return out


def scan(levels, scene_names, wanted, relays, hidden):
    """What each scene's objects say, read once:
    - where they refer to a tracked thing: (scene, x, y, object, relayed);
      a chest refers to its loot table, not to what is in it, and `relays`
      maps such a table to the things it holds;
    - the doors leading to the side rooms the map does not draw (`hidden`),
      by the room they lead to."""
    found, doors = {}, {}
    needles = {h: len(h).to_bytes(4, "little") + h.encode() for h in hidden}
    for n, path in levels:
        scene = scene_names[n]
        if scene in OFF_WORLD:
            continue
        f = list(UnityPy.load(path).files.values())[0]
        externals = [os.path.basename(e.path) for e in f.externals]
        objects = f.objects
        transforms, by_id = {}, {}
        for o in objects.values():
            if o.type.name in ("Transform", "RectTransform"):
                try:
                    t = o.read()
                except Exception:
                    continue
                transforms[t.m_GameObject.path_id] = t
                by_id[o.path_id] = t
        for o in objects.values():
            if o.type.name != "MonoBehaviour":
                continue
            raw = o.get_raw_data()
            go_id = struct.unpack_from("<iq", raw, 0)[1]
            t = transforms.get(go_id)
            if t is None:
                continue
            hits = set()
            for i in range(28, len(raw) - 12, 4):
                fid, target = struct.unpack_from("<iq", raw, i)
                if 0 < fid <= len(externals) and externals[fid - 1] == "resources.assets":
                    if target in wanted:
                        hits.add((target, False))
                    for thing in relays.get(target, ()):
                        hits.add((thing, True))
            leads = [h for h, needle in needles.items() if h != scene and needle in raw]
            if not hits and not leads:
                continue
            name = t.m_GameObject.read().m_Name
            if name.startswith(NOISE):
                continue
            x, y = world(t, by_id)
            for target, relayed in hits:
                found.setdefault(target, []).append((scene, x, y, name, relayed))
            for h in leads:
                doors.setdefault(h, []).append((scene, x, y))
    return found, doors


def world(t, by_id):
    """An object's place in its scene: its transforms down from the root."""
    chain = []
    while t is not None:
        chain.append(t)
        t = by_id.get(t.m_Father.path_id) if t.m_Father.path_id else None
    x = y = 0.0
    sx = sy = 1.0
    for t in reversed(chain):
        x += t.m_LocalPosition.x * sx
        y += t.m_LocalPosition.y * sy
        sx *= t.m_LocalScale.x
        sy *= t.m_LocalScale.y
    return x, y


def places(found, scenes, doors):
    """The thing's spot on the map, as fractions of its scene's bounds (from
    the left and the bottom): where an object gives it (rather than checks for
    it), directly rather than through a loot table. In a side room the map
    does not draw, the spot is the door that leads there, marked approximate."""
    def rank(f):
        return (f[4], not any(g in f[3] for g in GIVERS))

    for scene, x, y, _, _ in sorted(found, key=rank):
        approx = False
        if scene not in scenes:
            door = next((d for d in doors.get(scene, []) if d[0] in scenes), None)
            if door is None:
                continue
            scene, x, y, approx = door[0], door[1], door[2], True
        b = scenes[scene]
        fx, fy = (x - b[0]) / (b[2] - b[0]), (y - b[1]) / (b[3] - b[1])
        if not (0 <= fx <= 1 and 0 <= fy <= 1):
            continue
        p = {"scene": scene, "x": round(fx, 4), "y": round(fy, 4)}
        if approx:
            p["approximate"] = True
        return [p]
    return []
