# Peekle's Homebrew cask. This file is the source; the tap
# (MarselNet86/homebrew-tap) carries the copy that release.yml renders on
# every tag: the version and the checksum of the dmg fill the two
# placeholders and the result is attached to the release as `peekle.rb`.
# Edit here, never in the tap. tech.md 6.27, "Homebrew".
cask "peekle" do
  version "__VERSION__"
  sha256 "__SHA256__"

  url "https://github.com/MarselNet86/peekle/releases/download/v#{version}/Peekle-mac-universal.dmg"
  name "Peekle"
  desc "Claude Code, answered from the notch"
  homepage "https://github.com/MarselNet86/peekle"

  livecheck do
    url :url
    strategy :github_latest
  end

  # 13.0 or newer, the bundle's minimumSystemVersion.
  depends_on macos: :ventura

  app "Peekle.app"
  # The command that wires the hooks ships inside the bundle. tech.md 6.27.
  binary "#{appdir}/Peekle.app/Contents/MacOS/peekle"

  # The bundle is signed ad hoc, not notarized (tech.md R-7). Homebrew marks
  # what it downloads as quarantined, and Gatekeeper would refuse the app on
  # its first launch; clearing the mark is what makes this the supported way
  # to install.
  postflight_steps do
    run "/usr/bin/xattr", args: ["-cr", "{{appdir}}/Peekle.app"]
  end

  uninstall quit: "app.peekle.overlay"

  zap trash: [
    "~/Library/Application Support/peekle",
    "~/Library/Caches/app.peekle.overlay",
    "~/Library/Caches/peekle",
    "~/Library/HTTPStorages/app.peekle.overlay",
    "~/Library/Preferences/app.peekle.overlay.plist",
    "~/Library/WebKit/app.peekle.overlay",
  ]

  caveats <<~EOS
    Peekle listens to Claude Code through hooks. Wire them in once:

      peekle init

    Then open Peekle. `peekle uninstall` takes the hooks out again.
  EOS
end
