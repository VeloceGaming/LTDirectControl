from paths import GAME_DIR
from pathlib import Path
import struct
game=GAME_DIR
with (game/'bundle.game_data').open('rb') as f:
    def integer():return struct.unpack('<I',f.read(4))[0]
    for _ in range(integer()):
        ext=f.read(integer()).decode();path=f.read(integer()).decode();n=integer()
        if ext in ['style','ui'] and path.endswith('/main'):
            text=f.read(n).decode()
            for needle in ['color_icon_button','camera_button']:
                at=text.find(needle)
                print(ext,path,needle,text[max(0,at-50):at+1400])
        else:f.seek(n,1)
