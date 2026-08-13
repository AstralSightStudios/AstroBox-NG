import os

PROJECT_ROOT = "/Users/yulimfish/Documents/AstroBox-NG"
APP_PATH = os.path.join(
    PROJECT_ROOT, "src-tauri/target/release/bundle/macos/AstroBox.app"
)
BACKGROUND = os.path.join(
    PROJECT_ROOT, "src-tauri/modules/app/resources/dmgbg@2x.png"
)
OUTPUT = os.path.join(PROJECT_ROOT, "AstroBox.dmg")

volume_name = "AstroBox"
format = "UDZO"
size = "160M"
files = [APP_PATH]
symlinks = {"Applications": "/Applications"}
background = BACKGROUND

window_rect = ((100, 100), (400, 640))
icon_locations = {
    "AstroBox.app": (200, 164),
    "Applications": (200, 450),
}
icon_size = 120
text_size = 14

os.environ["DMGBUILD_OUTPUT"] = OUTPUT
