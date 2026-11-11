class Ferrocast < Formula
  desc "Fast, open-source launcher for macOS with local file conversion"
  homepage "https://github.com/drumst0ck/ferrocast"
  url "https://github.com/drumst0ck/ferrocast/archive/refs/tags/v0.10.1.tar.gz"
  sha256 "10890d39a13c8c0f6698cac19d7f9bb4ad244109a6488112c7611b9f351e3fe8"
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
      Screen capture and recording need macOS 14 and Screen Recording permission.
      Update with `brew upgrade ferrocast`; the symlink keeps working.

      Apple Translation (offline translating of selected text) needs macOS 26 and the Xcode or
      Command Line Tools 26 SDK at install time. Without them everything else works and the
      Apple engine stays hidden in Settings → Translate.
    EOS
  end

  test do
    app = prefix/"Ferrocast.app"
    assert_path_exists app/"Contents/MacOS/ferrocast"
    assert_match "dev.ferrocast.launcher", (app/"Contents/Info.plist").read
    system "codesign", "--verify", "--strict", app
  end
end
