# Linux distribution, step by step

The agreed minimum build system is **Ubuntu 24.04**. Build a DEB there and qualify that same package on Ubuntu 24.04,
Ubuntu 26.04 and Debian 13. Build the RPM separately on **Fedora 43** and qualify it on Fedora 43 and Fedora 44.
Both x86_64 and ARM64 are in scope. Flatpak is outside this release's scope.

Native DEB/RPM packages with signed package-manager repositories follow the current
[OpenAI Linux distribution model](https://learn.chatgpt.com/docs/linux/linux-app). ARM64 uses a native ARM64 builder:
an x86_64 host with Podman does not become an ARM64 builder. GitHub provides
[native `ubuntu-24.04-arm` runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
if that is the chosen build route. Desktop qualification still requires each architecture.

The earlier host development DEB and AppImage were built on Ubuntu 26.04 and need GLIBC 2.43. The first native
Ubuntu 24.04/x86_64 DEB and Fedora 43/x86_64 RPM now require GLIBC 2.39, with extracted-app fake-engine checks
passing on Ubuntu 26.04's Wayland and XWayland backends. These remain development artifacts. The helpers below
write baseline builds into separate directories and reject newer binaries with incompatible GLIBC requirements.
The x86_64 DEB development revisions `0local`/`0local.1` have passed container dependency, install, upgrade, removal/purge, data
preservation and reinstall checks on Ubuntu 24.04, Ubuntu 26.04 and Debian 13. Fedora 43 and Fedora 44 have passed
the equivalent RPM checks. Signed APT metadata and RPM/repomd rehearsals also passed using ephemeral test keys.
Native ARM64 builds, the clean desktop matrix, real engines and production signing/hosting remain separate steps.

Development revision `0local.2` refreshes both packages with the approved maintainer contact. Its package metadata,
changelogs and checksums have been verified; the application binaries match the previously tested `0local.1`
binaries. The full lifecycle records above cover `0local`/`0local.1`.

Revision `0local.3` added Fedora system-policy support and passed the full five-OS lifecycle matrix, upgrading
from `0local.2`. Revision `0local.4` additionally preserves certificate verification when reapplying system policy.
Both formats still require GLIBC 2.39.
Both `0local.4` packages passed the 23-check extracted-app smoke suite on each of the host's Wayland and
XWayland backends; their checksums and dependency lists were also verified.

On the Ubuntu 26.04/x86_64 GNOME development computer, the installed `0local.3` DEB passed isolated desktop
checks through its registered desktop entry: Add engine, real Secret Service token storage and restoration after
quit/relaunch, Unicode/multiline input, clipboard copy/paste under XWayland, image attachment processing, large
history rendering/scrolling, resize/full-screen commands, reconnect and real microphone capture/stop. Both
Wayland and XWayland were exercised. This computer has development tools, so it is not a clean-customer VM.
Native keyboard automation was not delivered by GNOME; keyboard bindings were checked through GPUI dispatch.
Wayland clipboard ownership still needs a real compositor input event and remains a manual check. The image
processing check bypassed the file picker, so normal portal/file-picker use also remains a manual check. Real
engine access was deferred by the owner; desktop logout/reboot has not been tested.

Copyright/license work is deferred; [the notice template](../packaging/COPYRIGHT.template) will be completed later.
Development packaging remains available without it. Lintian's remaining error is the missing owner-approved
copyright notice. Rpmlint reports the development license
placeholder and a static OpenSSL crypto-policy warning. The local native-tls patch restores the complete OpenSSL
`system_default` policy after the upstream builder, preserving any stricter caller protocol bounds. The actual
client HTTPS/WSS transports passed 24 checks each on Fedora 43 and Fedora 44: allowed TLS 1.2/1.3 connections,
untrusted/expired/wrong-host certificate rejection, legacy TLS and policy-excluded cipher rejection, and a stricter
TLS 1.3 minimum plus FUTURE policy. A configuration requesting `VerifyMode=-Peer` still rejects untrusted
certificates. The static warning detects the upstream cipher-setting symbol even though
the system policy is subsequently restored; it has been reviewed against this runtime evidence.
The fake-engine desktop smoke suite uses loopback HTTP and does
not establish real-engine TLS, keyring, microphone or clean-desktop qualification.

## 1. Install the local builder

On your Ubuntu machine, from any directory:

```bash
sudo apt update
sudo apt install podman uidmap passt fuse-overlayfs
podman info --format '{{.Host.Security.Rootless}}'
```

The last command should print `true`. Run Podman as your regular user. Do not prefix the build commands with sudo.
If Podman reports missing subordinate UID/GID ranges, inspect `grep "^$(id -un):" /etc/subuid /etc/subgid` and
follow the [rootless setup documentation](https://github.com/containers/podman/blob/main/docs/tutorials/rootless_tutorial.md).
Do not invent ranges that overlap other users. Desktop VM tools are a separate setup step.

## 2. Build the x86_64 development packages

From your local repository root:

```bash
shells/linux/release/container-build.sh deb
shells/linux/release/container-build.sh rpm
```

These install the pinned Rust toolchain into isolated Ubuntu 24.04/Fedora 43 builder images, build from source with
Cargo.lock, and generate native dependency metadata. They mount the repository and a Cargo cache; they do not
forward your desktop, audio, keyring, SSH agent or home directory. Native targets are separated by distro and CPU
under `build/native/`, so the old Ubuntu 26.04 target directory is never reused.

For the current version, the outputs on this machine are:

```text
dist-dev/ubuntu24.04-x86_64/optchat_0.1.0-0local_amd64.deb
dist-dev/fedora43-x86_64/optchat-0.1.0-0local.fc43.x86_64.rpm
```

Each package has `.sha256` and `.build.json` sidecars. The directory also records the base and builder image
identities. Base images are refreshed from their versioned upstream tags; set `OPTCHAT_BUILD_REFRESH=1` to bypass
image-layer cache and refresh installed builder packages for a release. These are repeatable build recipes and
provenance records, not a promise of bit-for-bit reproducibility. Package builds may take several minutes.

## 3. Build ARM64 natively

On an ARM64 Linux machine or native ARM64 runner with rootless Podman, use the same two commands from step 2.
The output directories become `ubuntu24.04-aarch64` and `fedora43-aarch64`; Debian names the package architecture
`arm64`, while RPM names it `aarch64`:

```text
dist-dev/ubuntu24.04-aarch64/optchat_0.1.0-0local_arm64.deb
dist-dev/fedora43-aarch64/optchat-0.1.0-0local.fc43.aarch64.rpm
```

Do not reuse an x86_64 binary or infer ARM64 support from a cross-compiled file. Record the actual native builder
and perform ARM64 desktop qualification before publishing ARM64 support.

The [Linux development packages workflow](../../../.github/workflows/linux-packages.yml)
uses GitHub's native `ubuntu-24.04` and `ubuntu-24.04-arm` runners with these same baseline containers. GitHub
[requires a manually dispatched workflow to exist on the default branch](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow).
While this work remains on `fabiha/linux-appimage`, relevant source pushes to that branch trigger the builds
directly; no default-branch merge is needed for this initial ARM64 qualification. After the workflow is reviewed
and merged to the default branch, push the release source, open
**Actions → Linux development packages → Run workflow**, and select the release branch. Each native DEB/RPM job
builds `0local` and `0local.1`, checks package lifecycle including upgrade,
and uploads the packages, checksums, build reports and qualification logs as a development artifact. It does not
sign packages or publish a release. Both RPM jobs also run the HTTPS/WSS policy checks on Fedora 43 and 44.
Downloaded packages still require the desktop qualification in step 5.

Run the same TLS qualification locally after building the baseline RPM:

```bash
shells/linux/release/check-tls.sh
```

It builds the transport-only example from the actual client crate, creates temporary certificates and loopback
servers, and records results under `build/qualification/`. Test trust roots and stricter OpenSSL configurations
apply only to disposable client processes; this does not change the host's trust store or crypto policy.

## 4. Test dependencies and package lifecycle

On each architecture, run these against its matching packages:

```bash
shells/linux/release/check-install.sh dist-dev/ubuntu24.04-x86_64/optchat_0.1.0-0local_amd64.deb
shells/linux/release/check-install.sh dist-dev/fedora43-x86_64/optchat-0.1.0-0local.fc43.x86_64.rpm
```

Replace paths with the ARM64 paths on that machine. The DEB script tests Ubuntu 24.04, Ubuntu 26.04 and Debian 13;
the RPM script tests Fedora 43 and 44. It uses disposable rootless containers to test native architecture, dependency
resolution, install, executable layout/mode, direct shared-library resolution, removal/purge, preserved user-data
sentinel and reinstall. Each run saves verified package snapshots, a SHA256 manifest, logs and OS image identities
under `build/qualification/`, so all target systems test the same bytes even if a package is rebuilt concurrently.
Public repository metadata and downloaded dependencies are cached per OS/architecture under
`build/qualification-cache/`. Every check still starts in a fresh container and resolves packages through the
distribution's signed repositories. Test containers retain OptChat documentation that minimal OCI images normally omit.

To test an initial upgrade, rebuild with `container-build.sh deb --revision 0local.1` (and likewise `rpm`), then
pass the newer package as the first argument and the original `0local` package as the second argument to
`check-install.sh`. If no previous package is supplied, upgrade is explicitly untested.

These containers have no desktop; they cannot qualify rendering, portals, audio or secure token storage.

## 5. Qualify the actual desktop packages

Create clean desktop VMs/hardware for each target OS and architecture. They should not have Rust, compiler headers
or your development configuration. Copy the package and checksum, then install through the distro package manager:

```bash
# Ubuntu/Debian, in the directory containing the downloaded package:
sha256sum -c optchat_0.1.0-0local_amd64.deb.sha256
sudo apt install ./optchat_0.1.0-0local_amd64.deb
optchat

# Fedora:
sha256sum -c optchat-0.1.0-0local.fc43.x86_64.rpm.sha256
sudo dnf install ./optchat-0.1.0-0local.fc43.x86_64.rpm
optchat
```

Use the correct ARM64 filenames there. Also launch from the application menu, test clipboard/files/microphone,
and connect through the normal UI to a real test engine. Confirm secure token persistence after reboot, upgrade,
remove/reinstall, and behavior with a locked/unavailable keyring. Test the supported Wayland/X11 paths explicitly.

For fake-engine checks, copy/clone the test source and install Deno 2.x (no Rust build is needed), then run:

```bash
shells/linux/test-native-package.sh /absolute/path/to/optchat.deb
# or an RPM; extraction uses rpm2cpio/cpio or the existing native Fedora builder image
shells/linux/test-native-package.sh /absolute/path/to/optchat.rpm
```

The suite extracts into temporary files and tests the packaged executable with isolated tokens/preferences; it does
not exercise the installed launcher or real Secret Service. Installed-menu and normal-UI tests remain necessary.
Use [QUALIFICATION.md](QUALIFICATION.md) to keep ten OS/architecture sign-off records. Run Lintian on the DEB
and Rpmlint on the RPM and review all findings before stable release.

## 6. Supply real release metadata and rebuild candidates

The approved default maintainer is `Fabiha Tasneem <fabiha@reframe.so>` for both package formats and their changelogs.
Override it with `--maintainer` or `OPTCHAT_PACKAGE_MAINTAINER` when needed; the command-line option takes precedence.

Copyright/license work is deferred for now. Continue with development revisions (`0local` or `0local.N`) while
[the notice template](../packaging/COPYRIGHT.template) is completed later. The template is a drafting aid and is
not automatically included in packages. Production revisions still need the project's approved notice, and RPM
also needs its approved SPDX license expression. Do not choose a new project license merely to fill packaging fields.
Include applicable third-party dependency notices in the approved notice; embedded fonts and icons already have
their notices copied by the helpers. Commit the release source and update the Cargo version for each new app version.

Keep the approved notice inside the repository so the builder can read it. Then, using real values:

```bash
shells/linux/release/container-build.sh deb \
  --revision 1 --copyright-file path/to/approved-notice
shells/linux/release/container-build.sh rpm \
  --revision 1 --copyright-file path/to/approved-notice \
  --license-expression 'PROJECT_APPROVED_SPDX_EXPRESSION'
```

These are command templates: replace the notice path and SPDX expression once approved. Revision `1` is a release
candidate, not an automatic stable publication. Repeat the checks on the final candidate bytes on both architectures.
The helpers refuse production revisions on the wrong OS, with reserved example contacts or without a notice.

## 7. Prepare signed repositories

Choose HTTPS APT and DNF addresses and a project-owned GPG signing key. Keep the private key outside the checkout
and make its ownership, passphrase, backup, expiry and rotation procedure explicit. Use its full fingerprint.
Signing runs on the chosen signing workstation; no private key is needed in the build containers or ARM64 builder.

For APT (requires GPG, dpkg-dev and the qualified packages from both architectures):

```bash
python3 shells/linux/release/apt_repository.py \
  dist-dev/ubuntu24.04-x86_64/optchat_0.1.0-1_amd64.deb \
  dist-dev/ubuntu24.04-aarch64/optchat_0.1.0-1_arm64.deb \
  --key "$OPTCHAT_SIGNING_KEY" --suite stable \
  --output dist-dev/apt-v0.1.0
```

For DNF (requires GPG, RPM 4.19+ including rpmsign/rpmkeys, createrepo_c):

```bash
python3 shells/linux/release/rpm_repository.py \
  dist-dev/fedora43-x86_64/optchat-0.1.0-1.fc43.x86_64.rpm \
  dist-dev/fedora43-aarch64/optchat-0.1.0-1.fc43.aarch64.rpm \
  --key "$OPTCHAT_SIGNING_KEY" --base-url "$OPTCHAT_RPM_URL" \
  --output dist-dev/rpm-v0.1.0
```

The RPM helper signs copies, verifies them using a temporary RPM database, generates per-CPU repositories and signs
`repomd.xml`. The original unsigned build artifacts remain unchanged; distribute the signed RPM copies with their
new checksums. The APT helper signs `InRelease` and `Release.gpg`. Both export the public key and full fingerprint.
Neither uploads files or changes this machine's repository configuration or trusted keys.

Use a private test repo and `--allow-development` for rehearsal packages. Development packages never belong in
the stable customer repository. Test hosted metadata and bad-signature rejection on fresh clients before promotion.

## 8. Publish and give customers native installation instructions

Upload versioned DEB and **signed** RPM files, checksums, changelog and support instructions to a download page or
GitHub Release. A ZIP is unnecessary; customers download the matching package directly. Separately host the
prepared APT/DNF directory contents over HTTPS at the chosen addresses. Deploy payloads and hashed metadata first,
switch signed entry metadata last, and keep older referenced files available during updates.

Publish the signing fingerprint independently from the repository key download. Ubuntu/Debian customer setup uses
a repository-specific `Signed-By` keyring. After setting the real HTTPS `OPTCHAT_APT_URL`, verify the fingerprint
against the independently published value before installing the key:

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL "$OPTCHAT_APT_URL/optchat-archive-keyring.asc" -o /tmp/optchat-archive-keyring.asc
gpg --show-keys --with-fingerprint /tmp/optchat-archive-keyring.asc
sudo install -m 0644 /tmp/optchat-archive-keyring.asc /etc/apt/keyrings/optchat.asc
printf 'Types: deb\nURIs: %s\nSuites: stable\nComponents: main\nSigned-By: /etc/apt/keyrings/optchat.asc\n' "$OPTCHAT_APT_URL" | sudo tee /etc/apt/sources.list.d/optchat.sources
sudo apt update
sudo apt install optchat
```

The architecture is selected by APT automatically. Fedora uses the generated `optchat.repo` with both package and
metadata signature checks enabled. For Fedora, after verifying the fingerprint:

```bash
curl -fsSL "$OPTCHAT_RPM_URL/optchat-archive-keyring.asc" -o /tmp/OPTCHAT-KEY
gpg --show-keys --with-fingerprint /tmp/OPTCHAT-KEY
sudo install -m 0644 /tmp/OPTCHAT-KEY /etc/pki/rpm-gpg/OPTCHAT-KEY
curl -fsSL "$OPTCHAT_RPM_URL/optchat.repo" -o /tmp/optchat.repo
# Inspect that its HTTPS baseurl is the intended address before installing it.
sudo install -m 0644 /tmp/optchat.repo /etc/yum.repos.d/optchat.repo
sudo dnf install optchat
```

The owner supplies the real repository URLs before these commands are customer-ready. Packages contain only the
client; customers connect it to their engine. Do not distribute fake tokens, development state or a fake engine as
part of a production installation.

## 9. Verify managed updates

Create a higher-version candidate on the same baselines, qualify it, sign it and publish it to a private test channel.
Ubuntu/Debian: `sudo apt update && sudo apt install --only-upgrade optchat`.
Fedora: `sudo dnf upgrade --refresh optchat`.
Confirm the intended version was installed and workspaces/tokens persist. Rehearse recovery to a known-good version
before stable promotion. Customers update through the distro package manager; no app self-updater is required.

## 10. Release only after sign-off

Complete the [publish gate](QUALIFICATION.md#publish-gate), record limitations and publish the actual supported OS,
architecture and desktop backends. Packaging helpers and automated smoke tests do not establish production
readiness. AppImage remains available as a separate portable/development channel; the existing Ubuntu 26.04 image
must not be advertised as compatible with the Ubuntu 24.04 minimum.
