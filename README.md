

for kde

how to build

```
cmake -S kde/kwin-effect -B target/kwin-effect-build
cmake --build target/kwin-effect-build
```

will get

```
target/kwin-effect-build/kwin/effects/plugins/quickerradialeffect.so
```

then

```
mkdir -p ~/.local/lib/qt6/plugins/kwin/effects/plugins
cp target/kwin-effect-build/kwin/effects/plugins/quickerradialeffect.so \
  ~/.local/lib/qt6/plugins/kwin/effects/plugins/

mkdir -p ~/.local/share/kwin/effects/quickerradialeffect
cp kde/kwin-effect/quickerradialeffect.json \
  ~/.local/share/kwin/effects/quickerradialeffect/metadata.json
```

you can then check it with

```
kpackagetool6 --type KWin/Effect --list | grep quickerradialeffect
```

Then:

Re-log into your Plasma Wayland session, or reload KWin.
Open System Settings > Desktop Effects.
Enable Quicker Radial Effect (the name comes from quickerradialeffect.json).
