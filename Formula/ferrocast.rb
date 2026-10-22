class Ferrocast < Formula
  desc "Fast, open-source launcher for macOS with local file conversion"
  homepage "https://github.com/drumst0ck/ferrocast"
  url "https://github.com/drumst0ck/ferrocast/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "9dea77b68f12ab7d03a552b101c027d52dc4389a00e6bde4c1dc07c05ce86a73"
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
