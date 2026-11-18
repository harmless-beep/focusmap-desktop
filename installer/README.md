# Installer

`Product.wxs` is the WiX v3 source for `FocusMap-setup.msi`. It installs
`FocusMap.exe` into `Program Files\FocusMap` and adds Start Menu and Desktop
shortcuts. The install is per-machine (Windows will ask for permission).

To rebuild the MSI with the free WiX 3.14 tools:

```powershell
# From the repository root, with WiX 3.14's candle.exe / light.exe on PATH:
cargo build --release -p focusmap-app
candle.exe -dSourceExe="$PWD\target\release\focusmap-app.exe" -out obj\ installer\Product.wxs
light.exe -ext WixUIExtension.dll -out installer\output\FocusMap-setup.msi obj\Product.wixobj
```

Update the `Version` attribute in `Product.wxs` for new releases; the upgrade
code stays the same so installs upgrade cleanly.

The ready-to-share files are in `installer/output/` (gitignored):

- `FocusMap.exe` — portable build, no install needed
- `FocusMap-setup.msi` — installer
