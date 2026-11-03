# Import
You import game files through the `Import` page. This page can download the vanilla files, read the game installed on this device, decrypt pack files you provide and sort existing files on disk.

## Obtaining Files
There are four methods for obtaining game files: `Download`, `Device`, `Pack`, and `Raw`.

### Download
Fetch the vanilla game files from a file host. This method requires the following:
- **A source link** to an asset payload served from a public bucket. Paste it into the link field and tap "Check for Files Now".
- **Accepting the acknowledgment** that appears the first time a link is entered. A provider always overwrites every game file and clears the import manifest.

With `File Handling` set to `Prompt` the app checks the link on launch and offers newer files as a download. `Ignore` never checks on launch.

### Device
Read the game installed on this same device. This method requires the following:
- **The Battle Cats installed** for the region you pick, or any of them with `All Regions`.
- **Keys & IV** added through `Settings > General > Manage Keys` so the app can decrypt the game's `.pack` file format.

`Update Only` reads the installed update data, which every device can do without root. `All Content` also copies the game's own data folder, which holds the files the game downloaded after install, and that needs a rooted device that grants this app root access when asked.

If the update data is missing the app was modified or installed in an unusual way. The job stops there unless `Settings > Import > Ignore Modified App` is on, in which case the base app is imported instead.

### Pack
Decrypt `.pack` files provided to the app. This method requires the following:
- **Encrypted `.pack` and `.list` files** on your device. Tap "Select Files" and pick them together in the system file picker; they are copied into the app's own storage for the job.
- **Keys & IV** added through `Settings > General > Manage Keys` so the app can decrypt the game's `.pack` file format.

Choose the region the packs came from and start the job.

### Raw
Copy raw game files you provide to the app. This method requires the following:
- **Raw Game Files** picked with the system file picker, or a `.zip`, `.tar` or `.zst` archive holding them.

Alternatively, you can copy files straight into the `game` folder on your device yourself.

## Sorting Files
Setting the `Raw` source to `Game Folder` targets the `game` folder itself: the import flattens the entire database into `raw` and then runs the database sorter on the flattened files, so files you copied into `game` by hand land where the app expects them.
