"""Fetch spacecraft maps of the major moons, Pluto, Charon and Ceres.

Downloads global mosaics made from Galileo, Voyager, Cassini, New Horizons
and Dawn images, published by the USGS Astrogeology Science Center and
NASA's Planetary Data System (public domain). Each is averaged down to a
2048 x 1024 map in the layout Worldline's renderer uses: equirectangular,
longitude 0° in the middle, east to the right, north up, in the body's IAU
coordinates.

Every map is placed from its own label (corner coordinates, pixel size and
radius), not from its stated center longitude: some labels say "centered on
180°" for an image that starts there. Each output pixel is the plain average
of the photographed source pixels whose centers fall inside it (0 means "no
data" in these products). Nothing is filled in: what no spacecraft
photographed stays black.

Writes lossless PNG files to crates/worldline-app/assets/textures/.

Run from anywhere:  python tools/fetch_spacecraft_maps.py [Body ...]
Uses only the Python standard library. Streams about 1.7 GB of source
images without saving them, which takes several minutes.
"""

import math
import re
import struct
import sys
import time
import urllib.request
import zlib
from pathlib import Path

OUT_DIR = Path(__file__).resolve().parent.parent / "crates" / "worldline-app" / "assets" / "textures"
WIDTH, HEIGHT = 2048, 1024
USGS = "https://planetarymaps.usgs.gov/mosaic/"
CKAN = "https://astrogeology.usgs.gov/ckan/dataset/"

# Body, output file, source image (GeoTIFF, or PDS3 image with its label at
# the start), and the image's ISIS label (None for PDS3).
MAPS = [
    ("Io", "2k_io.png", USGS + "Io_GalileoSSI-Voyager_Global_Mosaic_1km.tif",
     CKAN + "b9102ce8-3ee4-4848-8558-3dab5f52091a/resource/ca342229-fa0e-446b-8b5b-fb31bd1acdda/download/io_galileossi-voyager_global_mosaic_1km.lbl"),
    ("Europa", "2k_europa.png", USGS + "Europa_Voyager_GalileoSSI_global_mosaic_500m.tif",
     CKAN + "4080036f-afc5-422e-abe9-1c0c8e4f98ea/resource/db62f55a-9d03-474e-a349-1fd7d8f0d5fc/download/europa_voyager_galileossi_global_mosaic_500m.lbl"),
    ("Ganymede", "2k_ganymede.png", USGS + "Ganymede_Voyager_GalileoSSI_global_mosaic_1km.tif",
     CKAN + "57cad6e2-ed52-4b99-9d44-afbb9def6450/resource/32769bd3-7a00-4aa6-9ce8-52126cbd4384/download/ganymede_voyager_galileossi_global_mosaic_1km.lbl"),
    ("Callisto", "2k_callisto.png", USGS + "Callisto_Voyager_GalileoSSI_global_mosaic_1km.tif",
     CKAN + "a80abd68-7ed9-440e-829a-76376779164f/resource/66433621-ca46-4563-994f-c615931adb42/download/callisto_voyager_galileossi_global_mosaic_1km.lbl"),
    ("Titan", "2k_titan.png", USGS + "Titan_ISS_P19658_Mosaic_Global_4km.tif",
     CKAN + "8ee17e4e-26c6-4e22-9c23-bc9a4c7ed35e/resource/1c67c148-e7d4-4e4e-8a3e-f74ad9f1d32c/download/titan_iss_p19658_mosaic_global_4km.lbl"),
    ("Enceladus", "2k_enceladus.png", USGS + "Enceladus_Cassini_mosaic_global_110m.tif",
     CKAN + "30bff65e-56bb-4fd1-bd04-edd9bc2e77d0/resource/9f60754a-c7ef-4a96-81e6-50dcc1c4ad6d/download/enceladus_cassini_mosaic_global_110m.lbl"),
    ("Tethys", "2k_tethys.png", USGS + "Tethys_Cassini_mosaic_global_293m.tif",
     CKAN + "e40296c1-b4bf-46d8-86af-4b6cf0301b0c/resource/42693df3-3928-477d-8626-653d41699864/download/tethys_cassini_mosaic_global_293m.lbl"),
    ("Dione", "2k_dione.png", USGS + "Dione_Cassini_Voyager_mosaic_global_154m.tif",
     CKAN + "acb98ae6-ec50-42df-9a74-142d177bbe6d/resource/f79f032d-34d0-4527-8992-7e4d8a560535/download/dione_cassini_voyager_mosaic_global_154m.lbl"),
    ("Rhea", "2k_rhea.png", USGS + "Rhea_Cassini_Voyager_mosaic_global_417m.tif",
     CKAN + "22bc1015-d9c9-4212-86c3-e42061b204d4/resource/8ad50a33-b29b-4a85-9edc-759f00ee0641/download/rhea_cassini_voyager_mosaic_global_417m.lbl"),
    ("Iapetus", "2k_iapetus.png", USGS + "Iapetus_Cassini_Voyager_mosaic_global_783m.tif",
     CKAN + "6ac8ecfb-36e7-4113-8d16-c92ba857c3d7/resource/6d691cac-f7dd-4fe9-a70e-4b67cbddfdcf/download/iapetus_cassini_voyager_mosaic_global_783m.lbl"),
    ("Triton", "2k_triton.png", USGS + "Triton_Voyager2_ClrMosaic_GlobalFill_600m.tif",
     CKAN + "445b4c39-e87a-4e4d-88a8-e48d8e755c5c/resource/a79dfc3b-0267-435f-8ea0-5784a27c96bf/download/triton_voyager2_clrmosaic_globalfill_600m.lbl"),
    ("Pluto", "2k_pluto.png", USGS + "Pluto_NewHorizons_Global_Mosaic_300m_Jul2017_8bit.tif",
     CKAN + "a5f1b7f4-9822-4697-a201-e23ef4bd3e16/resource/d654329d-bc64-43c3-a844-fcede2d1ce77/download/pluto_newhorizons_global_mosaic_300m_jul2017_8bit.lbl"),
    ("Charon", "2k_charon.png", USGS + "Charon_NewHorizons_Global_Mosaic_300m_Jul2017_8bit.tif",
     CKAN + "93827f6c-8feb-42b6-98e6-b0ce57c7d2c8/resource/dd59f4b5-7998-4905-98ce-a74d3b177b2f/download/charon_newhorizons_global_mosaic_300m_jul2017_8bit.lbl"),
    ("Ceres", "2k_ceres.png", "https://sbnarchive.psi.edu/pds3/dawn/fc/DWNCHCFC2_2/DATA/CE_HAMO_G_00N_180E_EQU_CLR.IMG",
     None),
]


def request(url, start=None, end=None):
    headers = {"User-Agent": "Worldline map fetch"}
    if start is not None:
        headers["Range"] = f"bytes={start}-{end}"
    return urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=300)


def read_range(url, start, length):
    with request(url, start, start + length - 1) as response:
        data = response.read()
    assert len(data) == length, (url, start, length, len(data))
    return data


def read_exact(stream, n):
    data = bytearray()
    while len(data) < n:
        chunk = stream.read(n - len(data))
        if not chunk:
            raise EOFError("source image ended early")
        data += chunk
    return bytes(data)


def value(label, key):
    """The first `key = value` in a label, without units or quotes."""
    match = re.search(rf"^\s*{key}\s*=\s*\"?([^\s<\"]+)", label, re.M)
    if not match:
        raise KeyError(key)
    return match.group(1)


class Layout:
    """Where a source image lies on the body.

    `west` is the east longitude of the left edge of column 0 and `north`
    the latitude of the top edge of row 0, both in degrees; `dx` and `dy`
    are a pixel's width and height in degrees (east and south positive).
    """

    def __init__(self, width, height, bands, west, north, dx, dy):
        self.width, self.height, self.bands = width, height, bands
        self.west, self.north, self.dx, self.dy = west, north, dx, dy


def isis_layout(label, width, height, bands):
    """From an ISIS label's Mapping group. Its projection x grows eastward
    whichever way the longitudes are numbered, and x = 0 is the center
    longitude; UpperLeftCornerX/Y is the outer corner of the first pixel.
    For a sphere, equirectangular and simple cylindrical are the same.
    """
    assert (int(value(label, "Samples")), int(value(label, "Lines"))) == (width, height)
    radius = float(value(label, "EquatorialRadius"))
    assert radius == float(value(label, "PolarRadius")), "maps of spheres only"
    if "CenterLatitude" in label:
        assert float(value(label, "CenterLatitude")) == 0.0
    center = float(value(label, "CenterLongitude"))
    if value(label, "LongitudeDirection") == "PositiveWest":
        center = -center
    degrees = 180.0 / math.pi / radius
    pixel = float(value(label, "PixelResolution")) * degrees
    west = center + float(value(label, "UpperLeftCornerX")) * degrees
    north = float(value(label, "UpperLeftCornerY")) * degrees
    return Layout(width, height, bands, west, north, pixel, pixel)


def pds3_layout(label):
    """From a PDS3 label (Dawn's, from DLR). The map's origin (the center
    longitude on the equator) is at sample SAMPLE_PROJECTION_OFFSET and line
    LINE_PROJECTION_OFFSET, counting pixel centers from 0.
    """
    assert value(label, "POSITIVE_LONGITUDE_DIRECTION") == "EAST"
    assert value(label, "A_AXIS_RADIUS") == value(label, "C_AXIS_RADIUS"), "maps of spheres only"
    pixel = 1.0 / float(value(label, "MAP_RESOLUTION"))
    center = float(value(label, "CENTER_LONGITUDE"))
    sample = float(value(label, "SAMPLE_PROJECTION_OFFSET"))
    line = float(value(label, "LINE_PROJECTION_OFFSET"))
    return Layout(
        int(value(label, "LINE_SAMPLES")),
        int(value(label, "LINES")),
        int(value(label, "BANDS")),
        center - (sample + 0.5) * pixel,
        (line + 0.5) * pixel,
        pixel,
        pixel,
    )


def tiff_image(url):
    """Size, bands and where the pixels start in an uncompressed, 8-bit
    GeoTIFF whose rows are stored in order (as USGS publishes them)."""
    head = read_range(url, 0, 16)
    order = {b"II": "<", b"MM": ">"}[head[:2]]
    assert struct.unpack(order + "H", head[2:4])[0] == 42, "classic TIFF"
    ifd = struct.unpack(order + "I", head[4:8])[0]
    count = struct.unpack(order + "H", read_range(url, ifd, 2))[0]
    entries = read_range(url, ifd + 2, count * 12)
    tags = {}
    for k in range(count):
        tag, kind, n = struct.unpack(order + "HHI", entries[12 * k: 12 * k + 8])
        size = {3: 2, 4: 4}.get(kind)
        raw = entries[12 * k + 8: 12 * k + 12]
        if size is None:
            continue
        fmt = order + ("H" if size == 2 else "I")
        if n * size <= 4:
            tags[tag] = [struct.unpack(fmt, raw[i * size:(i + 1) * size])[0] for i in range(n)]
        else:
            where = struct.unpack(order + "I", raw)[0]
            data = read_range(url, where, n * size)
            tags[tag] = [struct.unpack(fmt, data[i * size:(i + 1) * size])[0] for i in range(n)]
    width, height = tags[256][0], tags[257][0]
    bands = tags.get(277, [1])[0]
    assert set(tags[258]) == {8}, "8-bit samples"
    assert tags[259] == [1], "uncompressed"
    assert bands == 1 or tags.get(284) == [2], "bands stored one after another"
    offsets, counts = tags[273], tags[279]
    assert sum(counts) == width * height * bands
    assert all(offsets[k + 1] == offsets[k] + counts[k] for k in range(len(offsets) - 1)), "rows stored in order"
    return width, height, bands, offsets[0]


def pds3_image(url):
    """Its label, and where the pixels start."""
    head = read_range(url, 0, 65536).decode("latin-1")
    label = head[: re.search(r"^END\s*$", head, re.M).end()]
    record = int(value(label, "RECORD_BYTES"))
    image = int(value(label, r"\^IMAGE"))
    assert value(label, "SAMPLE_BITS") == "8" and value(label, "SAMPLE_TYPE") == "UNSIGNED_INTEGER"
    return label, (image - 1) * record


def shrink(url, layout, start):
    """Average the source image into WIDTH x HEIGHT pixels, one list of rows
    per band."""
    # Each source column goes to the output column holding its center. A map
    # that repeats its first column after 360° has the repeat left out.
    period = 360.0 / layout.dx
    columns = [c for c in range(layout.width) if c + 0.5 < period]
    target = [
        int(((layout.west + (c + 0.5) * layout.dx + 180.0) % 360.0) * WIDTH / 360.0) % WIDTH
        for c in columns
    ]
    runs = []
    for c, i in zip(columns, target):
        if runs and runs[-1][0] == i and runs[-1][2] == c:
            runs[-1][2] = c + 1
        else:
            runs.append([i, c, c + 1])
    assert len({i for i in target}) == WIDTH, "source coarser than the output"
    rows_in = [
        min(HEIGHT - 1, max(0, int((90.0 - layout.north + (r + 0.5) * layout.dy) / 180.0 * HEIGHT)))
        for r in range(layout.height)
    ]
    assert len(set(rows_in)) == HEIGHT, "source coarser than the output"

    # Sums and counts of the photographed source pixels in each output
    # pixel. In these 8-bit products 0 means "no data", not black terrain
    # (some also pad their edge with it), so zeros are left out.
    sums = [[[0] * WIDTH for _ in range(HEIGHT)] for _ in range(layout.bands)]
    counts = [[[0] * WIDTH for _ in range(HEIGHT)] for _ in range(layout.bands)]
    with request(url, start, start + layout.width * layout.height * layout.bands - 1) as stream:
        for band in range(layout.bands):
            for r in range(layout.height):
                row = read_exact(stream, layout.width)
                acc, n = sums[band][rows_in[r]], counts[band][rows_in[r]]
                for i, a, b in runs:
                    acc[i] += sum(row[a:b])
                    n[i] += b - a - row.count(0, a, b)
    out = []
    for band in range(layout.bands):
        rows = []
        for j in range(HEIGHT):
            acc, n = sums[band][j], counts[band][j]
            rows.append(bytes(
                (acc[i] + n[i] // 2) // n[i] if n[i] else 0 for i in range(WIDTH)
            ))
        out.append(rows)
    return out


def png(path, bands):
    """Write gray (one band) or RGB (three) rows as an 8-bit PNG, each row
    Paeth-filtered."""
    channels = len(bands)
    stride = WIDTH * channels
    raw = bytearray()
    previous = bytes(stride)
    for j in range(HEIGHT):
        if channels == 1:
            row = bands[0][j]
        else:
            row = bytes(v for pixel in zip(*(b[j] for b in bands)) for v in pixel)
        filtered = bytearray(stride + 1)
        filtered[0] = 4
        for k in range(stride):
            left = row[k - channels] if k >= channels else 0
            up = previous[k]
            corner = previous[k - channels] if k >= channels else 0
            p = left + up - corner
            pa, pb, pc = abs(p - left), abs(p - up), abs(p - corner)
            predicted = left if pa <= pb and pa <= pc else (up if pb <= pc else corner)
            filtered[k + 1] = (row[k] - predicted) & 0xFF
        raw += filtered
        previous = row

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 0 if channels == 1 else 2, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def main():
    wanted = set(sys.argv[1:])
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for body, name, url, label_url in MAPS:
        if wanted and body not in wanted:
            continue
        began = time.time()
        if label_url is None:
            label, start = pds3_image(url)
            layout = pds3_layout(label)
        else:
            width, height, bands, start = tiff_image(url)
            with request(label_url) as response:
                label = response.read().decode("latin-1")
            layout = isis_layout(label, width, height, bands)
        size = layout.width * layout.height * layout.bands / 1e6
        print(
            f"{body}: {layout.width} x {layout.height} x {layout.bands} ({size:.0f} MB), "
            f"left edge {layout.west:.4f} deg E, top edge {layout.north:.4f} deg, "
            f"{layout.dx * 1000:.3f} millidegrees per pixel",
            flush=True,
        )
        png(OUT_DIR / name, shrink(url, layout, start))
        print(f"  -> {name}: {(OUT_DIR / name).stat().st_size / 1e6:.2f} MB in {time.time() - began:.0f} s", flush=True)


if __name__ == "__main__":
    main()
