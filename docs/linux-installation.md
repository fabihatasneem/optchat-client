# Install OptChat on Linux

OptChat is a desktop app for connecting to an OptChat engine. You will need a client token from your engine
administrator to load your workspaces. Installing this app does not install the engine.

**Current status: development preview.** Prebuilt DEB and RPM packages are available from successful GitHub Actions
builds. A stable Linux release and public APT/DNF update repositories have not been published yet. Package
installation has been tested across the systems below; full desktop and real-engine qualification is still in progress.
For preview testing, ask for a test workspace.

You need a graphical Linux desktop, internet access for dependencies and engine connections, permission to install
software with `sudo`, and an unlocked desktop password wallet/keyring for saving your engine token. You do not
need to clone the repository or install Rust, Deno or Podman to use a prebuilt package.

## 1. Choose the package for your computer

Check your distribution and version in your system's **About** settings. You can also run:

```bash
cat /etc/os-release
uname -m
```

The first command shows the system name/version, including `PRETTY_NAME`. The second shows the processor type.
Choose one download:

- **Ubuntu 24.04/26.04 or Debian 13, `x86_64`:** `linux-deb-x86_64`. The installer ends in `_amd64.deb`.
- **Ubuntu 24.04/26.04 or Debian 13, `aarch64`/`arm64`:** `linux-deb-aarch64`. The installer ends in `_arm64.deb`.
- **Fedora 43/44, `x86_64`:** `linux-rpm-x86_64`. The installer ends in `.x86_64.rpm`.
- **Fedora 43/44, `aarch64`/`arm64`:** `linux-rpm-aarch64`. The installer ends in `.aarch64.rpm`.

`amd64` means the standard 64-bit Intel/AMD package; it works on both brands. `arm64` and `aarch64` name the
64-bit ARM architecture. Other distributions/versions and 32-bit machines are outside this package test matrix.

## 2. Download and extract it

1. Sign in to GitHub. GitHub requires a signed-in account with repository read access to download workflow artifacts.
2. Open [Linux development packages](https://github.com/fabihatasneem/optchat-client/actions/workflows/linux-packages.yml).
3. Open a completed run with a green success check. A running or failed build is not the installer to choose.
4. Scroll to **Artifacts** and download the name you chose in step 1. Downloads named `linux-evidence-...` are
   maintainer logs and test evidence; choose the `linux-deb-...` or `linux-rpm-...` download for installation.
5. Extract the ZIP into its own folder using your file manager. Open `INSTALL.md` if it is present: it contains
   commands with the exact package filename for that download.

New downloads contain one installer, its `.sha256` checksum, `INSTALL.md`, and a `build-info/` folder. Keep the
installer and its checksum together. GitHub artifacts are retained for 14 days in this workflow. If a download has
expired, ask the maintainer for a fresh successful build or a package/checksum pair.
If you do not use GitHub, the maintainer can send you the same installer ZIP directly; continue at step 3 after
extracting it.

**If an older ZIP contains `dist-dev/` and `build/`:** open `dist-dev/`, then its matching system/processor folder
(for example `ubuntu24.04-x86_64` or `fedora43-aarch64`). Select the newer package containing `0local.1` and its
matching `.sha256` file. Copy those two files into a new, empty folder. The `0local` package and copies under
`build/qualification/` are used for upgrade tests; you do not need them to install the app.

See [GitHub's artifact download instructions](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/download-workflow-artifacts)
if you cannot find the Artifacts section.

## 3. Check the download and install

Open a terminal **in the folder containing the installer and its checksum**. Many file managers offer
**Open in Terminal** when you right-click an empty area. That folder should contain only one OptChat installer.

### Ubuntu or Debian

Run these commands one at a time:

```bash
sha256sum -c ./optchat_*.deb.sha256
```

Continue only if the result ends in **OK**. Then install:

```bash
sudo apt update
sudo apt install ./optchat_*.deb
```

### Fedora

Check the file first:

```bash
sha256sum -c ./optchat-*.rpm.sha256
```

Continue only if the result ends in **OK**. Then install:

```bash
sudo dnf install ./optchat-*.rpm
```

The package manager downloads required system libraries and installs the app. Review the proposed changes and
confirm when prompted. When `sudo` asks for your password, it is your Linux account password; the terminal may
show no characters while you type it.

The package name is **`optchat`**, although the source repository is named `optchat-client`. The `./` in the install
command tells the package manager to use your downloaded file. Installing by name alone needs a configured OptChat
repository, which is not available yet. These CI previews are unsigned; if your system requires a signed RPM,
request an approved signed package from the maintainer rather than disabling signature checks.

## 4. Open OptChat and add your engine

1. Open your desktop's application menu, search for **OptChat**, and launch it. You can also run `optchat` in a
   terminal as your regular user.
2. Ask your engine administrator for a client token for your permitted workspaces. It starts with `oc1.`.
   If you manage the engine, open its `/engine` admin page and create a token under **Client tokens**.
3. In OptChat, open **Workspaces → Add engine**, paste the token and add the engine. Its permitted workspaces
   connect automatically.
4. Quit and reopen OptChat to check that your desktop keyring has saved the token. If the desktop asks you to
   unlock its password wallet, unlock it to allow secure token storage.

Do not launch the app with `sudo`. An empty workspace list before adding an engine is expected. Keep client tokens
private; do not include them in screenshots, support messages or logs.

Useful shortcuts: **Ctrl+K** opens the command palette, **F11** toggles full screen, and **Ctrl+Q** quits.

## Updating and removing the app

For now, download a newer preview for the same system/processor, verify its checksum and repeat the matching
installation command above. Quit OptChat before updating. Downloads do not update automatically, and ordinary
system updates will not fetch OptChat updates until a signed OptChat package repository is published and configured.
New CI workflow runs use increasing development revisions. Reruns of the same workflow run keep the same version;
use a later successful run when looking for an update.

To uninstall on Ubuntu/Debian:

```bash
sudo apt remove optchat
```

On Fedora:

```bash
sudo dnf remove optchat
```

Removing the package keeps your preferences and engine tokens. Preferences normally live in
`~/.config/optchat`; tokens are stored separately in your desktop keyring.

## If something does not work

- **No artifact or download button:** sign in to GitHub, select a successful completed run, and check that its
  artifacts have not expired. Ask the maintainer for a fresh download if needed.
- **Checksum says FAILED or a file is missing:** download and extract the package and its matching checksum again.
  Run the command from their shared folder; do not continue installing a file that fails the check.
- **Wrong architecture / cannot execute the file:** repeat `uname -m` and choose the matching download from step 1.
- **Unable to locate package:** use the downloaded file with `./` as shown above. `optchat-client` is not the
  installed package name.
- **Missing libraries or a GLIBC error:** check your OS version and package choice. Use the supported DEB/RPM for
  your system. Earlier experimental AppImages require newer Ubuntu 26.04 libraries and have different compatibility.
- **Token does not stay saved:** make sure your desktop password wallet/keyring is running and unlocked. GNOME
  Keyring is one supported Secret Service provider. Do not use a plain-text token file as a workaround.
- **The window does not open:** run `optchat` in a terminal on your graphical desktop and copy the error text for
  the maintainer. The app needs working graphics drivers; installing a package in a headless server or SSH session
  does not create a desktop session.
- **No workspaces appear after adding the engine:** confirm with the administrator that the token is valid, grants
  workspace access and points to an engine reachable from your computer.

For help, contact the person who supplied your installer or
[Fabiha Tasneem](mailto:fabiha@reframe.so). Include your Linux version, `uname -m` result, package filename and
the error text. Leave out client tokens and other credentials.
