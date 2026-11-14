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

Enable a mod from its page to browse the game with it applied. Only one mod is mounted at a time, and the enabled mod is remembered between launches.

### Workflow
The whole loop runs on this device:
1. **Import** the game files through the `Import` page or `Settings > Files > Source`.
2. **Create** a mod through "Add Mod" and tap "Enable" on its page.
3. **Edit** the files you want to change, as described below.
4. **Check** your changes on the `Cats`, `Enemies`, `Stages` and `Sandbox` pages, which all show the game with your mod applied.
5. **Export** the mod, as described at the bottom of this page.

### Edit
Tap "Edit" on the page of an enabled mod. This opens a popup that lists every file a mod can change, sorted into `Cats`, `Enemies`, `Stages` and `Other`. The button is disabled while the mod is disabled.

Tap a folder to open it, or type into the search field to narrow the list down. Tap an entry to select it, then tap "Edit File".

What "Edit File" opens depends on the entry:
- **Stats and settings**: Opens a form that lists every value by name. Type a number, flip a switch or pick an option, and the change is written to your mod right away.
- **Names and descriptions**: Opens the same form with one text field per line of the entry.
- **Everything else**: Copies the file from the game into your mod if your mod does not have it yet, then opens your mod's copy in the `Files` page. An image opens there too, with its "Replace PNG" button.

The form's "Sync With "game"" button asks you to tap it a second time, then puts the vanilla values back.

In the popup's list, an entry your mod changes is tinted yellow and an entry your mod adds is tinted green. Select a tinted entry and tap "Sync With "game"" under "Edit File" to put that entry back to vanilla, which also asks for a second tap.

Either way this is how a vanilla file gets into your mod, since a mod only holds the files it changes. The other pages pick your changes up once you close the form or save the file.

### Files
The `Files` page lists every mounted folder, and your mod is one of them while it is enabled. A text file is edited as text right on the page. An image shows a "Replace PNG" button that swaps it for a PNG picked with the system file picker.

Tap "Save Changes" to write your edits. They are also saved when you open another file, switch folders or leave the page.

### Names
There are two ways the popup can name its entries: `Resolved` and `Raw`. The mode can be changed under `Settings > Files > Editor > Editor Mode`.
- **Resolved**: Shows what each entry means in the game, such as a unit's name and form. The default option.
- **Raw**: Shows the real file names and IDs.

`Other > Files` holds every file that has no entry of its own. They are sorted by file type and always shown by their real file name, so any file in the game can be brought into your mod.

## Export
You can export a mod under the `Mods` page by selecting your mod and tapping "Export", in which you are given 3 options:
- **APK**: Patch a `.apk` file you select and export it with the mod applied. Requires Keys & IV. Split `.xapk` files are only supported in the desktop app.
- **BCM**: Export into a `.bcm` archive, which is an archive format optimized for low file sizes.
- **Pack**: Export into the game's proprietary `.pack` and `.list` format for pack-specific injection. Requires Keys & IV.

Exports land in the `exports` folder.
