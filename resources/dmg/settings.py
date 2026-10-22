# dmgbuild settings for the Ferrocast disk image (used by scripts/dmg.sh).
# Window 660x400 pt; the coordinates match resources/dmg/background.html, which leaves a label chip
# under each icon. Values passed with -D: app (the .app bundle), background, volume_icon.
import os.path

app = defines.get("app", "dist/Ferrocast.app")  # noqa: F821 (defines is injected by dmgbuild)
background = defines.get("background", "resources/dmg/background.png")  # noqa: F821
icon = defines.get("volume_icon", "resources/Ferrocast.icns")  # noqa: F821

format = "UDZO"
filesystem = "HFS+"
files = [app]
symlinks = {"Applications": "/Applications"}

default_view = "icon-view"
window_rect = ((200, 140), (660, 400))
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
sidebar_width = 0

arrange_by = None
icon_size = 128
text_size = 12
label_pos = "bottom"
show_icon_preview = False
show_item_info = False
icon_locations = {
    os.path.basename(app): (180, 170),
    "Applications": (480, 170),
}
