"""The game's Addressables catalog (StreamingAssets/aa/catalog.json): which
asset each GUID names. An item's picture is an asset reference by GUID; the
asset's file name is the sprite's name in the bundle.

The catalog packs its keys, buckets and entries as base64 binary: a key is a
type byte then its value (0 an ASCII string, 1 a UTF-16 one); bucket i holds
the entries of key i; an entry is seven integers, the first the index of its
internal id."""
import base64, json, os, struct


def assets(path):
    """GUID → the asset's path, for every GUID key of the catalog."""
    d = json.load(open(path, encoding="utf-8"))
    keys = base64.b64decode(d["m_KeyDataString"])
    buckets = base64.b64decode(d["m_BucketDataString"])
    entries = base64.b64decode(d["m_EntryDataString"])
    ids = d["m_InternalIds"]

    def key(at):
        kind = keys[at]
        if kind not in (0, 1):
            return None
        (n,) = struct.unpack_from("<i", keys, at + 1)
        return keys[at + 5:at + 5 + n].decode("ascii" if kind == 0 else "utf-16-le")

    out = {}
    (count,) = struct.unpack_from("<i", buckets, 0)
    at = 4
    for _ in range(count):
        offset, n = struct.unpack_from("<ii", buckets, at)
        first = struct.unpack_from("<i", buckets, at + 8)[0] if n else None
        at += 8 + 4 * n
        k = key(offset)
        if first is not None and k and len(k) == 32 and all(c in "0123456789abcdef" for c in k):
            (internal,) = struct.unpack_from("<i", entries, 4 + first * 28)
            out[k] = ids[internal]
    return out


def sprite_name(path):
    """A sprite's name from its asset's path: the file's name."""
    return os.path.splitext(os.path.basename(path))[0]
