/*
 * Single source for the install commands shown on the website (home hero block, CTA dialog, and later
 * the docs install page). Mirrors the Homebrew block of the README; keep them identical.
 */
export const repo = 'https://github.com/drumst0ck/ferrocast';

/** Always points at the newest stable release: the release workflow uploads a versionless copy. */
export const dmgUrl = `${repo}/releases/latest/download/Ferrocast.dmg`;

export const installCommand = `brew tap drumst0ck/ferrocast
brew install drumst0ck/ferrocast/ferrocast
ln -sfn "$(brew --prefix)/opt/ferrocast/Ferrocast.app" \\
  /Applications/Ferrocast.app
open /Applications/Ferrocast.app`;

export const sourceCommand = `git clone ${repo}
cd ferrocast
scripts/bundle.sh
open dist/Ferrocast.app`;
