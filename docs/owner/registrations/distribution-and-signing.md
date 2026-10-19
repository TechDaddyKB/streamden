# Distribution and signing: GitHub, release keys, AUR, Windows, Apple, domain

Last verified against official docs: 2026-10-18. Part of the [owner runbooks](../README.md). Most of this is P0 (build and release skeleton, spec `01-build-release.md`); several parts wait on open decisions (Q-B1 name, Q-B2 license, Q-B4 visibility, Q-P2 macOS, Q-G3 budget).

## Purpose

Everything the project needs to build, sign, publish and update the app on Windows, Linux (deb, rpm, AppImage, AUR) and macOS, and to host the code with the multi-agent PR workflow. These are human-only steps: they need your identity, your payment card, or your ID.

## Prerequisites

- [ ] Project email, password manager, authenticator/passkey (see [README](../README.md#general-rules)).
- [ ] Decisions: **Q-B1** (name: repo, AUR package, domain), **Q-B2** (license: SignPath, AUR `license=`), **Q-B4** (GitHub visibility), **Q-P2** (Apple), **Q-G3** (budget).

## Cost and limits (summary)

| Item | Cost | Notes |
|---|---|---|
| GitHub account | Free | Branch protection / rulesets on **private** repos needs a paid plan (see below). Public repos: free, with free Actions minutes. |
| GitHub Pro (if repo stays private) | about $4/month (verify at github.com/pricing) | Lists "Protected branches" for private repos. |
| AUR account | Free | |
| SignPath Foundation (Windows signing) | Free for OSS | OSI license required, publisher shown as SignPath Foundation. |
| Azure Artifact Signing | **$9.99/month** Basic (5,000 signatures, $0.005 each extra); $99.99/month Premium | Individuals only in USA/Canada (see below). |
| Apple Developer Program | **99 USD per membership year** (regional prices differ) | Needed for Developer ID signing and notarization. |
| Domain | typically about $10-20/year (verify at the registrar) | Cloudflare Registrar sells "at cost, without markup". |
| Tauri updater key, GPG key | Free | One-time local generation. |

---

## 1. GitHub

### 1.1 Account and repository

1. Sign in at <https://github.com> with the project identity. Add 2FA (authenticator app, plus a passkey or security key if you have one) at **Settings > Password and authentication** (verify on screen).
2. Decide personal account versus an **organization** (optional; an org is free and keeps the project independent of your personal account; org name needs Q-B1).
3. Create the repository: **+ > New repository** (verify on screen). Owner = account or org. Name = `streamden` (placeholder until Q-B1). Visibility per **Q-B4**. Do not add a README or license in the web form if you push the existing local repository.
4. Initialize and push from your machine (this is the Q-B4 `git init` step):

```bash
cd ~/Documents/Streamer.live
git init -b main
git add -A && git commit -m "chore: initial import"
git remote add origin git@github.com:<owner>/<repo>.git
git push -u origin main
```

5. SSH key for GitHub if you have none: `ssh-keygen -t ed25519 -C "<PROJECT_EMAIL>"`, then GitHub **Settings > SSH and GPG keys > New SSH key** (verify on screen). Or use `gh auth login`.

### 1.2 Important plan fact for Q-B4 (private repo during P0-P1)

- GitHub's plan documentation lists **Protected branches** under "advanced tools and insights in private repositories" for **Pro and Team**, not for Free. Rulesets are documented as available to Team and Enterprise customers, and branch protection is free on **public** repos. So on a **private repo with a Free account you cannot enforce** the PR-only, required-checks protection that multi-agent work relies on. Options: make the repo public from day one, pay for Pro/Team, or keep private and rely on discipline (not recommended).
- GitHub Free private repos get **2,000 Actions minutes per month**. The three-OS CI matrix (Windows and macOS minutes are billed at multiples of Linux minutes) can burn that quickly. Public repos get free Actions minutes. Both facts argue for revisiting the "private until v0.1" recommendation in Q-B4.

### 1.3 Branch ruleset for the multi-agent PR workflow

On a repo where rulesets are available:

6. Repository **Settings > Rulesets > Rulesets > New ruleset > New branch ruleset**.
7. **Ruleset name**: `protect-main`. **Enforcement status**: set it to **Active** (the default is Disabled).
8. **Target branches > Add a target**: include the **default branch**.
9. **Branch protections** (rule labels: verify on screen):
   - **Require a pull request before merging**. Required approvals: GitHub does not let an author approve their own PR. If every agent PR is authored with your own token, set approvals to **0** and rely on required checks plus your manual merge. If agents use a separate bot account or GitHub App, you can require **1** approval and approve as the owner.
   - **Require status checks to pass**: add each CI job name (`fmt`, `clippy`, `test`, `typecheck`, `codegen-drift`, and so on). Names only appear after the first CI run, so add them afterwards.
   - **Block force pushes**.
   - **Restrict deletions**.
   - **Require linear history** (optional, pairs with squash merges).
10. **Bypass list**: add yourself (repository admin) only if you want an emergency override. Click **Create**.
11. **Settings > General > Pull Requests**: allow only **squash merging** and enable **Automatically delete head branches**.
12. **Settings > Actions > General > Workflow permissions**: default to read-only; allow GitHub Actions to create pull requests only if release automation needs it.
13. Enable secret scanning and **push protection** (Settings > Code security; availability on private repos depends on the plan: verify on screen).

For `@codex review` and similar review bots you also install the vendor's GitHub app on the repo (not researched here; verify on screen in the vendor's settings).

### 1.4 CI secrets (Settings > Secrets and variables > Actions > New repository secret)

Create these only when the matching section below is done. Never store them anywhere else except the password manager.

| Secret | From | Section |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Tauri updater key | 2 |
| `TAURI_SIGNING_RPM_KEY`, `TAURI_SIGNING_RPM_KEY_PASSPHRASE` | GPG key | 3 |
| `AUR_SSH_PRIVATE_KEY` | AUR CI deploy key | 4 |
| SignPath token/org/project IDs | SignPath | 5A |
| `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET` | Azure | 5B |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` **or** `APPLE_API_ISSUER`/`APPLE_API_KEY`/`APPLE_API_KEY_PATH` content | Apple | 6 |
| `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID` | Cloudflare | [relay-cloudflare.md](relay-cloudflare.md) |

Use a protected **Environment** (for example `release`) with required reviewers for the release job so a rogue PR cannot read the signing secrets (**Settings > Environments**; verify on screen).

---

## 2. Tauri updater signing key (not a registration, but a one-time human step)

The updater (`tauri-plugin-updater`) refuses unsigned updates. **If you lose the private key you can never publish updates to installed copies.**

1. On your machine (outside the repo): `cargo tauri signer generate -w ~/.tauri/streamden.key` (or `pnpm tauri signer generate -w ...`). Choose a strong password.
2. Put the **public key** text into `tauri.conf.json` `plugins.updater.pubkey` (an agent task; the public key is safe to commit).
3. Store the **private key file and password** in the password manager (offline backup too). Add `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to GitHub Actions secrets. Tauri notes `.env` files do not work for these during builds.

## 3. GPG key for RPM signing and checksums

1. `gpg --full-generate-key` (ed25519, no expiry or a long one, user ID = project name and email).
2. Export the private key (`gpg --armor --export-secret-keys <ID>`) to the password manager and to the secret `TAURI_SIGNING_RPM_KEY` (+ passphrase). Publish the **public** key on the website and repo.
3. Also sign the release `SHA256SUMS` with it (agent task).

---

## 4. AUR (Arch User Repository)

Packages: `streamden-bin` (repackages the CI-built `.deb`), `streamden` (from a release tarball), optionally `streamden-git`. PKGBUILD drafts are in [tech-stack.md section 10.3](../../research/tech-stack.md). The package name is baked in at P0, so wait for **Q-B1**; the `license=` field waits for **Q-B2**.

1. **Register**: <https://aur.archlinux.org/register>. Fields: username, email, optionally a real name, PGP key and SSH public key (labels: verify on screen; the page itself blocked automated fetching). Confirm the email.
2. **Create an SSH key just for the AUR** (do not reuse another key):

```bash
ssh-keygen -t ed25519 -f ~/.ssh/aur -C "aur-<username>"
cat >> ~/.ssh/config <<'EOF'
Host aur.archlinux.org
  IdentityFile ~/.ssh/aur
  User aur
EOF
```

3. Log in to the AUR website, open **My Account**, and paste `~/.ssh/aur.pub` into the **SSH Public Key** field (multiple keys are separated by newlines). Save.
4. Set your git identity (commits are hard to rewrite after pushing): `git config --global user.name ...` and `user.email ...`.
5. Check the name is free by searching the AUR for `streamden`, `streamden-bin`.
6. **First push of a new package:**

```bash
git clone ssh://aur@aur.archlinux.org/streamden-bin.git   # "empty repository" warning is expected
cd streamden-bin
cp <path>/PKGBUILD <path>/streamden.install .
makepkg --printsrcinfo > .SRCINFO
makepkg -si        # test build and install locally
git add PKGBUILD .SRCINFO streamden.install
git commit -m "Initial import: streamden-bin <version>"
git push origin master    # the AUR only accepts the master branch
```

Rules to remember: only package files in the repo (no nested directories), `.SRCINFO` must be in the commit that you push, regenerate it with `makepkg --printsrcinfo`, and the branch must be `master` (rename with `git branch -M master` if needed).
7. **CI automation (later):** create a *second* SSH key, add its public part to your AUR account and its private part as the GitHub secret `AUR_SSH_PRIVATE_KEY`, then publish from the release workflow (`KSXGitHub/github-actions-deploy-aur` is widely used; its current version is unverified). Maintainer duties: answer comments and out-of-date flags.

---

## 5. Windows code signing

Without signing, Windows SmartScreen warns users who download the installer from a browser. Signing is not required to run the app. **EV certificates no longer get special SmartScreen treatment** (Microsoft removed it in 2024), and reputation builds per certificate, so sign every release with the same certificate.

### 5A. SignPath Foundation (free for OSS): recommended if the license is OSI-approved

Requirements (from SignPath's terms):

- An **OSI-approved open source license, with no commercial dual-licensing for any component** (this is why **Q-B2** matters; GPL-3.0-or-later qualifies).
- No malware or potentially unwanted programs; no proprietary non-open-source components (system libraries are fine).
- The project is **actively maintained and already released** in the form to be signed, with its functionality described on the download page.
- Builds are automated from the source repository; the signature confirms "a valid, automated build" of that source. Signing covers only your own code.
- A published **"Code signing policy"** page that uses that exact term and lists team roles (authors, reviewers, approvers), the free-signing credit, and a privacy policy.
- **Multi-factor authentication for every team member** on SignPath and the repository.
- Product name and version metadata set and enforced.
- SignPath wants "a certain verifiable reputation" for executables. Acceptance is discretionary.
- **The certificate is issued to SignPath Foundation**, so users see "SignPath Foundation" as publisher, not your name.

Steps: (1) reach a public release (v0.1) with the license set and the repo public; (2) publish the Code signing policy on the website/README; (3) apply via the "Apply" section of <https://signpath.org> (the path `/apply`; the form fields were not visible to automated fetching: verify on screen); (4) once accepted, integrate the SignPath GitHub Action into the release workflow (agent task); (5) store the SignPath API token in GitHub secrets.

### 5B. Azure Artifact Signing (formerly Trusted Signing): about $10/month

- Pricing: **Basic $9.99/month** for 5,000 signatures ($0.005 per extra); **Premium $99.99/month** for 100,000.
- Eligibility for **Public Trust** certificates: **individual developers must be located in the USA or Canada**, and identity details are pulled from your Azure billing account, which must be of type *Individual*. Organizations: USA, Canada, EU and UK, and Microsoft's current quickstart also lists Australia, New Zealand, Japan, South Korea, Singapore, Switzerland, Norway and Israel (secondary report; verify). A "3 years in business" rule was reportedly dropped in late 2025 (community report; verify). Validation can stall: community threads from mid-2026 show rejections.
- If you cannot qualify, use a traditional OV certificate from a commercial CA.

Setup outline (portal labels: verify on screen): create an Azure account and subscription; create a **resource group**; create an **Artifact Signing account** (Basic SKU); under it add **Identity validation** (complete the identity check) and a **Certificate profile** (Public Trust); create an **App registration** (service principal) and give it the *certificate profile signer* role; create a client secret. Add `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET` as GitHub secrets. Tauri signs through `signCommand` with `artifact-signing-cli` (`cargo install`), flags `-e` endpoint, `-a` account, `-c` certificate profile, `-d` description.

### 5C. Unsigned (alpha only)

Acceptable until the first public release. Document the SmartScreen "More info > Run anyway" step.

**Recommendation (from the research):** SignPath if the license is OSI-approved; otherwise Azure (if you qualify) or an OV certificate.

---

## 6. Apple Developer Program (macOS), pending **Q-P2**

Recommendation in Q-P2: **(b)** ship unsigned (ad-hoc signed) macOS builds until v0.5, then enroll if there is demand. Do **not** pay yet unless Q-P2 says (a).

If you enroll:

1. Prepare an **Apple Account with two-factor authentication**. **Individual**: needs your **legal name** (aliases delay approval) and the legal age of majority; your legal name becomes the developer identity on the certificate. **Organization**: needs a legal entity (no DBAs or trade names), a **D-U-N-S number**, legal binding authority, a work email on the organization's domain and a public website.
2. Enroll at <https://developer.apple.com/programs/enroll/> (or in the Apple Developer app). The fee is **99 USD per membership year**, due after your information is verified. No processing time is published.
3. Only the **Account Holder** can create a **Developer ID Application** certificate. Do it in **Certificates, Identifiers & Profiles** (or Xcode). That portal asks for a Certificate Signing Request (CSR). On a Linux-only setup you can create one without a Mac: `openssl req -new -newkey rsa:2048 -nodes -keyout devid.key -out devid.csr -subj "/CN=<Legal Name>/emailAddress=<email>"`, upload the CSR, download the `.cer`, then bundle key + certificate into a `.p12` with `openssl` (verify on screen; a Mac with Keychain Access is the easier path). The `.p12` password and file go into the password manager.
4. **Notarization credentials** (pick one; Tauri accepts either):
   - *App-specific password:* create it at your Apple Account's Sign-In and Security page (<https://account.apple.com>, Apple's support article HT204397; verify on screen). Env: `APPLE_ID`, `APPLE_PASSWORD` (the app-specific password), `APPLE_TEAM_ID`.
   - *App Store Connect API key (preferred, not tied to your personal password):* in App Store Connect create a **Team Key** (Users and Access > Integrations; verify on screen), download the `.p8` **once**, and note the **Key ID** and **Issuer ID**. Env: `APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH`.
5. For CI: base64-encode the `.p12` into `APPLE_CERTIFICATE`, set `APPLE_CERTIFICATE_PASSWORD` and `APPLE_SIGNING_IDENTITY` (the name shown as "Developer ID Application: ... (TEAMID)").
6. Notarization is **required** when using a Developer ID Application certificate, and a **free Apple Developer account cannot notarize**. Without any paid account, use ad-hoc signing (`"signingIdentity": "-"` in the Tauri config); macOS still makes users approve the app in **Privacy & Security**.

---

## 7. Domain (pending **Q-B1**)

1. Decide the name first, then check availability and conflicts (trademark search in your jurisdiction and WIPO/EUIPO/USPTO databases; this is not legal advice). "StreamDen" is close to "Streamer.bot", which is exactly the friction Q-B1 asks about.
2. Register through a registrar you trust. **Cloudflare Registrar** sells domains at cost with no markup, redacts WHOIS by default, offers one-click DNSSEC and auto-renews at the registry list price (TLD availability: check in the dashboard). Use the project email, turn on 2FA and keep auto-renew on.
3. The domain is needed for: the homepage and **privacy policy** (Google verification, Twitch Developer Services Agreement, Meta App Review), the SignPath **Code signing policy** page, docs hosting, the updater endpoint if you ever leave GitHub Releases, and the relay's custom hostname (domain on Cloudflare).
4. Add the domain to Google Search Console for the YouTube verification when you get there ([google-youtube.md](google-youtube.md)).

---

## Verification

- GitHub: open a throwaway PR and confirm you cannot push directly to `main` and that the merge button waits for the required checks.
- Updater key: `cargo tauri signer generate` produced both files; the public key is in `tauri.conf.json`; the secrets exist in Actions.
- AUR: after the first push the package appears at `https://aur.archlinux.org/packages/<name>`; `yay -S streamden-bin` (on a clean Arch container) installs it.
- Windows: right-click the signed installer, **Properties > Digital Signatures**, shows the publisher and a valid timestamp.
- macOS: `codesign -dv --verbose=4 "App.app"` and `spctl -a -vv "App.app"` report "accepted, source=Notarized Developer ID".

## Common pitfalls

- Registering the AUR package name before the product name is final (renaming an AUR package means a new package and a deletion request).
- Losing the Tauri updater key (updates become impossible for existing installs) or sharing it in chat.
- Assuming private-repo branch protection works on a Free GitHub account.
- SignPath rejects projects with any proprietary component or a dual license.
- Azure individual validation fails outside the USA/Canada.
- Apple legal-name mismatch delays enrollment; the Developer ID Application certificate can be created only by the Account Holder.
- Changing the signing certificate between Windows releases resets SmartScreen reputation.

## Policy notes

- SignPath: certificate is issued to SignPath Foundation; no liability for damages from signed software; discretionary acceptance.
- Apple: you accept the Apple Developer Program License Agreement; the developer identity shown is your legal name or entity.
- Arch: read the AUR submission guidelines before the first push; do not create duplicates of existing packages.

## Sources

- GitHub plans: <https://docs.github.com/en/get-started/learning-about-github/githubs-plans>
- GitHub rulesets: <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets> and <https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository>
- Tauri updater: <https://v2.tauri.app/plugin/updater/>
- Tauri macOS signing: <https://v2.tauri.app/distribute/sign/macos/>
- Tauri Windows signing: <https://v2.tauri.app/distribute/sign/windows/>
- Tauri AUR guide: <https://v2.tauri.app/distribute/aur/>
- AUR submission guidelines (wiki, mirrors): <https://wiki.archlinux.org/title/AUR_submission_guidelines>
- AUR registration: <https://aur.archlinux.org/register>
- SignPath terms: <https://signpath.org/terms>
- Azure Artifact Signing: <https://azure.microsoft.com/en-us/products/artifact-signing>, <https://learn.microsoft.com/en-us/azure/artifact-signing/overview>
- Apple Developer Program enrollment: <https://developer.apple.com/programs/enroll/>
- Developer ID and notarization: <https://developer.apple.com/developer-id/>
- Cloudflare Registrar: <https://developers.cloudflare.com/registrar/>
- Research: [tech-stack.md section 10](../../research/tech-stack.md)
