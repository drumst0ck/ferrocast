class Ferrocast < Formula
  desc "Fast keyboard launcher for macOS, written in Rust"
  homepage "https://github.com/drumst0ck/ferrocast"
  # Private repo for now: brew clones with your git credentials (a tarball URL would 404).
  # When the repo goes public, switch to the archive url + sha256 with scripts/update_formula.sh.
  url "https://github.com/drumst0ck/ferrocast.git", tag: "v0.1.0", revision: "9b34ff63ae0a37e824cf693a817d6e39bc849298"
  license "MIT"
  head "https://github.com/drumst0ck/ferrocast.git", branch: "master"

  depends_on "rust" => :build
  depends_on :macos

  def install
    # Built on the user's machine, so the app never carries a download quarantine flag and
    # needs no Apple Developer account. bundle.sh signs it ad-hoc.
    system "scripts/bundle.sh"
    prefix.install "dist/Ferrocast.app"
    bin.write_exec_script prefix/"Ferrocast.app/Contents/MacOS/ferrocast"
  end

  def caveats
    <<~EOS
      Ferrocast is an app bundle. Link it so Spotlight and Finder can see it:
        ln -sfn "#{opt_prefix}/Ferrocast.app" /Applications/Ferrocast.app
      Then open it and grant Accessibility when asked (System Settings → Privacy & Security).
      Update with `brew upgrade ferrocast`; the symlink keeps working.
    EOS
  end

  test do
    app = prefix/"Ferrocast.app"
    assert_path_exists app/"Contents/MacOS/ferrocast"
    assert_match "dev.ferrocast.launcher", (app/"Contents/Info.plist").read
    system "codesign", "--verify", "--strict", app
  end
end
