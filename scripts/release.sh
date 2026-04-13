#!/usr/bin/env bash
set -e

VERSION=$1
if [ -z "$VERSION" ]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.23.0"
  exit 1
fi

REPO="jackymint/cliTokenKill"
BRANCH="release/v${VERSION}"

echo "==> Creating release v${VERSION}"

# Update Cargo.toml version
sed -i '' "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml

# Build release
cargo build --release

# Commit changes
git add Cargo.toml Cargo.lock
git commit -m "Release v${VERSION}"

# Create and push tag
git tag "v${VERSION}"
git push origin "v${VERSION}"

# Create GitHub Release
echo "==> Creating GitHub Release..."
gh release create "v${VERSION}" \
  --title "v${VERSION}" \
  --notes "Release v${VERSION}" \
  --target main

# Wait for release to be available
sleep 2

# Generate SHA256
echo "==> Generating SHA256..."
TARBALL_URL="https://github.com/${REPO}/archive/refs/tags/v${VERSION}.tar.gz"
SHA256=$(curl -sL "${TARBALL_URL}" | shasum -a 256 | awk '{print $1}')
echo "SHA256: ${SHA256}"

# Create release branch
git checkout -b "${BRANCH}"
git push origin "${BRANCH}"

# Update homebrew formula
FORMULA_PATH="../homebrew-tap/Formula/ctk.rb"
if [ -f "$FORMULA_PATH" ]; then
  sed -i '' "s|url \".*\"|url \"${TARBALL_URL}\"|" "$FORMULA_PATH"
  sed -i '' "s/sha256 \".*\"/sha256 \"${SHA256}\"/" "$FORMULA_PATH"
  
  cd ../homebrew-tap
  git add Formula/ctk.rb
  git commit -m "Update ctk to v${VERSION}"
  git push origin main
  cd -
fi

# Create PR
echo "==> Creating PR..."
gh pr create \
  --base main \
  --head "${BRANCH}" \
  --title "Release v${VERSION}" \
  --body "Release v${VERSION}

SHA256: \`${SHA256}\`

See CHANGELOG for details." || echo "PR creation failed - create manually at: https://github.com/${REPO}/compare/main...${BRANCH}"

echo "==> Done! v${VERSION} released"
