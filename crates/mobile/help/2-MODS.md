# Mods
You can create, view and package mods using the `Mods` page.

## Initialize
To initialize a mod, there are two paths: `Automatic` or `Manual`.

### Automatic
Swipe in from the left edge of the `Mods` page to open the mod list and tap "Add Mod". The popup offers the following options:
- **New**: Creates a new mod folder for you using the name you provide.
- **BCM**: Import files from a `.zip` or `.bcm` archive picked with the system file picker.
- **Pack**: Import from an APK or from the game's proprietary `.pack` and `.list` files picked with the system file picker. Requires Keys & IV.

All mods initialized through this avenue will have a simple internal structure containing the following folders: `patch`, `icons`, and `loose`.

### Manual
Enter the `mods` folder inside the app's data folder (create it if it doesn't exist) and create a new folder inside of it. The folder name will be the name of your mod in the mod list.

## Develop
Within a mod's directory, all files act as an **override** that replaces the vanilla asset, and only that one asset. This means the **only** files you need inside this directory are your **custom** modified assets, not the whole database. This behavior mimics how the game behaves when custom assets are injected.

Enable a mod from its page to browse the game with it applied. Only one mod is mounted at a time, and the enabled mod is remembered between launches. The `Files` page lists every mounted folder, so a mod's files can be viewed and edited in place there.

## Export
You can export a mod under the `Mods` page by selecting your mod and tapping "Export", in which you are given 3 options:
- **APK**: Patch a `.apk` file you select and export it with the mod applied. Requires Keys & IV. Split `.xapk` files are only supported in the desktop app.
- **BCM**: Export into a `.bcm` archive, which is an archive format optimized for low file sizes.
- **Pack**: Export into the game's proprietary `.pack` and `.list` format for pack-specific injection. Requires Keys & IV.

Exports land in the `exports` folder.
