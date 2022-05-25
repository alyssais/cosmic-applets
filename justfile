rootdir := ''
prefix := '/usr'
clean := '0'
debug := '0'
vendor := '0'
target := if debug == '1' { 'debug' } else { 'release' }
vendor_args := if vendor == '1' { '--frozen --offline' } else { '' }
debug_args := if debug == '1' { '' } else { '--release' }
cargo_args := vendor_args + ' ' + debug_args


sharedir := rootdir + prefix + '/share'
iconsdir := sharedir + '/icons/hicolor/scalable/apps'
bindir := rootdir + prefix + '/bin'

audio_id := 'com.system76.CosmicAppletAudio'
graphics_id := 'com.system76.CosmicAppletGraphics'
network_id := 'com.system76.CosmicAppletNetwork'
power_id := 'com.system76.CosmicAppletPower'

all: _extract_vendor
    cargo build {{cargo_args}}

# Installs files into the system
install:
    # audio
    install -Dm0644 applets/cosmic-applet-audio/data/icons/{{audio_id}}.svg {{iconsdir}}/{{audio_id}}.svg
    install -Dm0644 applets/cosmic-applet-audio/data/{{audio_id}}.desktop {{sharedir}}/applications/{{audio_id}}.desktop
    install -Dm04755 target/release/cosmic-applet-audio {{bindir}}/cosmic-applet-audio

    # graphics
    install -Dm0644 applets/cosmic-applet-graphics/data/icons/{{graphics_id}}.svg {{iconsdir}}/{{graphics_id}}.svg
    install -Dm0644 applets/cosmic-applet-graphics/data/{{graphics_id}}.desktop {{sharedir}}/applications/{{graphics_id}}.desktop
    install -Dm04755 target/release/cosmic-applet-graphics {{bindir}}/cosmic-applet-graphics

    # network
    install -Dm0644 applets/cosmic-applet-network/data/icons/{{network_id}}.svg {{iconsdir}}/{{network_id}}.svg
    install -Dm0644 applets/cosmic-applet-network/data/{{network_id}}.desktop {{sharedir}}/applications/{{network_id}}.desktop
    install -Dm04755 target/release/cosmic-applet-network {{bindir}}/cosmic-applet-network

    # power
    install -Dm0644 applets/cosmic-applet-power/data/icons/{{power_id}}.svg {{iconsdir}}/{{power_id}}.svg
    install -Dm0644 applets/cosmic-applet-power/data/{{power_id}}.desktop {{sharedir}}/applications/{{power_id}}.desktop
    install -Dm04755 target/release/cosmic-applet-power {{bindir}}/cosmic-applet-power

# Extracts vendored dependencies if vendor=1
_extract_vendor:
    #!/usr/bin/env sh
    if test {{vendor}} = 1; then
        rm -rf vendor; tar pxf vendor.tar
    fi
