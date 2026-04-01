#!/bin/bash
set -e

VERSION=${1:-"0.1.0"}
APP_NAME="legend_shot"
APPDIR="${APP_NAME}.AppDir"

echo "=== Building ${APP_NAME} v${VERSION} ==="

# Build release
cargo build --release

# Create AppDir structure
rm -rf ${APPDIR}
mkdir -p ${APPDIR}/usr/bin
mkdir -p ${APPDIR}/usr/share/applications
mkdir -p ${APPDIR}/usr/share/icons/hicolor/256x256/apps
mkdir -p ${APPDIR}/usr/share/fonts/opentype/noto

# Copy binary
cp target/release/${APP_NAME} ${APPDIR}/usr/bin/

# Create desktop file
cat > ${APPDIR}/${APP_NAME}.desktop << EOF
[Desktop Entry]
Name=Legend Shot
Comment=Screenshot tool with annotations
Exec=${APP_NAME}
Icon=${APP_NAME}
Terminal=false
Type=Application
Categories=Utility;Graphics;
EOF

# Create icon using Python PIL
python3 -c "
from PIL import Image
img = Image.new('RGB', (256, 256), color='#4A90D9')
img.save('${APPDIR}/${APP_NAME}.png')
"

# Copy font (needed for Chinese support)
if [ -f /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc ]; then
    cp /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc ${APPDIR}/usr/share/fonts/opentype/noto/
fi

# Create AppRun
cat > ${APPDIR}/AppRun << 'EOF'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export FONT_PATH="${HERE}/usr/share/fonts"
exec "${HERE}/usr/bin/legend_shot" "$@"
EOF
chmod +x ${APPDIR}/AppRun

# Download appimagetool if not present
if [ ! -f appimagetool-x86_64.AppImage ]; then
    echo "Downloading appimagetool..."
    wget -q https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
    chmod +x appimagetool-x86_64.AppImage
fi

# Build AppImage
ARCH=x86_64 ./appimagetool-x86_64.AppImage ${APPDIR} ${APP_NAME}-${VERSION}-x86_64.AppImage

echo "=== Created ${APP_NAME}-${VERSION}-x86_64.AppImage ==="