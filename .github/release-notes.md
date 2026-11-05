The first release of gitid: see which git identity applies to any folder, and why, so commits never fall back to your OS login name again.

- **Identity:** who commits in a folder will be from, which file, rule or environment variable each value comes from, and a fix for each problem.
- **Rules:** identities for folders (`~/work`) or servers (your Gitea), over HTTPS or SSH.
- **Scan:** find the repos under a folder whose commits would fail or use an identity you didn't choose.
- **All config:** every value from system, global and repository config, and which one git uses.
- **The `gitid` command:** the same in a terminal, included with the app.

In English and Simplified Chinese.

## Downloads

| File | For |
| --- | --- |
| `gitid-gui-cli-macos-universal.tar.gz` | Macs, Apple silicon and Intel |
| `gitid-gui-cli-windows-x64-setup.exe` | Windows: installer, for your user only (no administrator rights) |
| `gitid-gui-cli-windows-x64-portable.zip` | Windows: no installer, run it from any folder |
| `gitid-gui-cli-linux-x64.deb` | Debian and Ubuntu 22.04 or newer |
| `gitid-gui-cli-linux-x64.AppImage` | Other Linux distributions |
| `gitid-cli-<platform>` | The `gitid` command alone, for machines without a desktop |

Each file's SHA-256 is shown next to it below. You'll also need [git](https://git-scm.com).

## First launch

- **macOS:** the app isn't notarized yet, so macOS blocks it the first time. Open it once, then choose **Open Anyway** under **System Settings > Privacy & Security**.
- **Windows:** the installer and app aren't signed yet, so SmartScreen may warn. Choose **More info > Run anyway**.
- **Linux AppImage:** make it executable first: `chmod +x gitid-gui-cli-linux-x64.AppImage`.

Then, on macOS, use **Settings > Command line** to install the `gitid` command. The Windows installer and the `.deb` add it for you.

gitid is free software under the GNU AGPL v3 or later. Install steps, the command line and building from source are in the [README](https://github.com/Late-Commit-Office/gitid#readme).
