# Releasing

Releases are built and published by `.github/workflows/release.yml`. Nothing
is built or signed on a personal machine.

## Publishing a release

1. Set the version in `Cargo.toml` (`[workspace.package] version`). Use plain
   `X.Y.Z`: Windows version info cannot hold a pre-release suffix.
2. In `CHANGELOG.md`, move the entries under `[Unreleased]` into a new
   `## [X.Y.Z] - YYYY-MM-DD` section and add its link at the bottom.
3. Commit and push to `main`. The Release workflow builds the installer on
   every push to `main`; wait for it and CI to pass.
4. Tag and push:

   ```sh
   git tag -a vX.Y.Z -m "bikey X.Y.Z"
   git push origin vX.Y.Z
   ```

The workflow checks that the tag matches `Cargo.toml` and that the changelog
has a section for it. Then it builds, signs (once SignPath is set up), and
publishes a GitHub Release with the installer, `SHA256SUMS.txt`, a build
provenance attestation, and the changelog section as release notes. With
signing on, each run waits for someone to approve two signing requests in
SignPath: the DLLs, then the installer.

## Setting up code signing (SignPath Foundation)

SignPath Foundation signs open-source projects for free. The certificate is
issued to SignPath Foundation, so Windows shows it as the publisher. A new
certificate still has to build up SmartScreen reputation, so early downloads
can show a warning even when signed.

1. **Apply** at <https://signpath.org>. Their conditions include an
   OSI-approved licence (MIT/Apache-2.0 qualify), a public repo, an existing
   release in the form to be signed (v0.1.0), and **MFA on GitHub and
   SignPath** for every team member.
2. **Add a code signing policy** to the README before or when you apply. The
   wording below is required by SignPath; keep it once approved:

   ```markdown
   ## Code signing policy

   Free code signing provided by [SignPath.io](https://about.signpath.io),
   certificate by [SignPath Foundation](https://signpath.org).

   - Committers and reviewers: [Bryan Le](https://github.com/levuthanhbinh)
   - Approvers: [Bryan Le](https://github.com/levuthanhbinh)

   Privacy: this program will not transfer any information to other networked
   systems unless specifically requested by the user or the person installing or
   operating it.
   ```

   Then replace the "not code-signed yet" note in the README, the notice in
   SECURITY.md, and the unsigned-build line in the release workflow's notes.
3. **Once approved**, in SignPath:
   - Create (or use) the project with slug `bikey` and link this GitHub
     repository as its trusted build system.
   - Add two artifact configurations, `dlls` and `installer`, from
     `.signpath/artifact-configurations/`.
   - Use the signing policy SignPath creates (slug `release-signing` by default)
     and make sure it requires manual approval.
   - Create an API token for a user with submitter rights.
4. **In GitHub** (Settings → Secrets and variables → Actions):
   - Secret `SIGNPATH_API_TOKEN`: the API token.
   - Variable `SIGNPATH_ORGANIZATION_ID`: your SignPath organisation ID.
   - Optional variables `SIGNPATH_PROJECT_SLUG` and `SIGNPATH_POLICY_SLUG` if
     your slugs differ from `bikey` and `release-signing`.

Signing turns on for tagged releases as soon as `SIGNPATH_ORGANIZATION_ID` is
set. The workflow then checks every signature before publishing.

Known gap: the uninstaller that Inno Setup embeds in the installer stays
unsigned, because the installer is signed after it is compiled.
