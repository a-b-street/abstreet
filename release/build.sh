#!/bin/bash
# Called by Github Actions workflow

set -e;

os=$1;
case $os in
    ubuntu-18.04)
        output="abst_linux";
        suffix="";
        ext="sh";
        ;;

    macos-latest)
        output="abst_mac";
        suffix="";
        ext="sh";
        ;;

    windows-latest)
        output="abst_windows";
        suffix=".exe";
        ext="bat";
        ;;

    *)
        echo "Wat? os = $os";
        exit 1;
esac

mkdir $output

cp release/play_abstreet.$ext release/ungap_the_map.$ext release/INSTRUCTIONS.txt $output

for name in game cli fifteen_min osm_viewer parking_mapper santa ltn; do
    bin="target/release/${name}${suffix}"
    if [[ "$output" = "abst_mac" ]]; then
        # If this errors or hangs, ensure you've imported and unlocked a
        # keychain holding a codesigning certificate with a Common Name
        # matching "Developer ID Application.*"
        codesign -fs "Developer ID Application" --timestamp -o runtime "$bin"
    fi
    cp "$bin" "$output";
done

mkdir $output/data
cp -Rv data/system $output/data/system
cp data/MANIFEST.json $output/data

case $os in
    ubuntu-18.04)
        # TODO Github will double-zip this, but if we just pass the directory, then the
        # chmod +x bits get lost
        zip -r $output $output
        rm -rf $output
        ;;

    macos-latest)
        ditto -c -k --keepParent $output $output.zip
        xcrun notarytool submit --wait \
            --apple-id $MACOS_DEVELOPER_APPLE_ID \
            --team-id $MACOS_DEVELOPER_TEAM_ID \
            --password $MACOS_DEVELOPER_APP_SPECIFIC_PASSWORD \
            $output.zip

        # TODO: staple the notarization so users can launch the app while
        # offline without warning. There's no way to staple the notarization to
        # raw binaries. To staple the notarization we need to adapting to a
        # .dmg, .pkg, or .app compatible installation. So until that happens,
        # users will need to be online the first time they launch the binary,
        # else they'll see the dreaded error:
        #
        # >  can’t be opened because Apple cannot check it for malicious software.

        rm -rf $output
        ;;

    windows-latest)
        # Windows doesn't have zip?!
        # TODO: can we use `7z a`?
        ;;

    *)
        echo "Wat? os = $os";
        exit 1;
esac
