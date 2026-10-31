# WinGet: mwo-dk.Bragd

The first version goes to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) by hand,
from these templates. Once Microsoft has listed it, `.github/workflows/winget.yml` files every
newer release with Komac, once a week.

Fill them in for a published release, from its `SHA256SUMS`:

```sh
export VERSION=X.Y.Z
sums=$(gh release download "v$VERSION" -R mwo-dk/bragd -p SHA256SUMS -O -)
export SHA_X64=$(awk -v f="Bragd_${VERSION}_x64-setup.exe" '$2 == f { print toupper($1) }' <<<"$sums")
export SHA_ARM64=$(awk -v f="Bragd_${VERSION}_arm64-setup.exe" '$2 == f { print toupper($1) }' <<<"$sums")
export DATE=$(gh release view "v$VERSION" -R mwo-dk/bragd --json publishedAt -q '.publishedAt[:10]')
out="manifests/m/mwo-dk/Bragd/$VERSION"
mkdir -p "$out"
for f in packaging/winget/*.yaml; do envsubst < "$f" > "$out/${f##*/}"; done
winget validate "$out"
```

Then copy `$out` into a fork of winget-pkgs at the same path and open the pull request titled
`New package: mwo-dk.Bragd version X.Y.Z`.
