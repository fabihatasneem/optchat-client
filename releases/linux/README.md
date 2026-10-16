# Linux installers

For Ubuntu or Debian on an Intel/AMD 64-bit computer, use the **[.deb installer](x86_64/optchat_0.1.0-0local.4_amd64.deb)**.
It is in `releases/linux/x86_64/` from the repository root.

- Ubuntu 24.04/26.04 or Debian 13: [DEB installer](x86_64/optchat_0.1.0-0local.4_amd64.deb).
- Fedora 43/44: [RPM installer](x86_64/optchat-0.1.0-0local.4.fc43.x86_64.rpm).

Open `x86_64/` in your file manager, right-click an empty area, and choose **Open in Terminal**.
Use the commands for your system:

## Ubuntu or Debian

```bash
sha256sum -c optchat_0.1.0-0local.4_amd64.deb.sha256
sudo apt update
sudo apt install ./optchat_0.1.0-0local.4_amd64.deb
```

## Fedora

```bash
sha256sum -c optchat-0.1.0-0local.4.fc43.x86_64.rpm.sha256
sudo dnf install ./optchat-0.1.0-0local.4.fc43.x86_64.rpm
```

The checksum must say **OK** before installing. Launch **OptChat** from your application menu, then choose
**Workspaces → Add engine** and paste the client token supplied by your engine administrator.
See the [Linux installation guide](../../shells/linux/README.md) for keyring setup, updates and troubleshooting.

## Build information

These are unsigned `0local.4` development packages copied from the existing Ubuntu 24.04 and Fedora 43 builds.
They have not been rebuilt from the merged source. Original `.build.json` reports and `.sha256` files are kept
beside each installer without changes. Smoke-test and lifecycle fields in those build-time reports record the
state at packaging time; subsequent checks are documented in the [qualification record](../../shells/linux/release/QUALIFICATION.md).

ARM64 installers and the newer `0local.7` preview are linked from the Linux installation guide.
This folder currently contains x86_64 packages only. For rebuilding and refreshing these files, see
the [maintainer guide](../../shells/linux/BUILDING.md). Keep each installer, checksum and build report together
and verify the checksum after copying a new build into this directory.
