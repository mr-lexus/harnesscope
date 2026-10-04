cask "harnesscope" do
  arch arm: "aarch64", intel: "x86_64"

  version "0.2.3"
  sha256 arm:   "781fa55db918d0cc9db159e58e866f22dd196ac2ed8316fffdbe9421209b66e8",
         intel: "6d92439203fca447badc39f03d6f9caa3fac2b97d575b29b01fcff93737b4016"

  url "https://github.com/mr-lexus/harnesscope/releases/download/v#{version}/harnesscope-v#{version}-#{arch}-apple-darwin.tar.gz"
  name "Harnesscope"
  desc "Local telemetry for AI coding workflows"
  homepage "https://github.com/mr-lexus/harnesscope"

  depends_on :macos

  binary "harnesscope"

  # Preserve telemetry databases and user configuration on uninstall.
  caveats <<~EOS
    This cask installs a prebuilt binary; it does not compile with Xcode.
    Start manually with: harnesscope server start
    Open the panel with: harnesscope ui
    brew services manages the formula, not this cask.
    If switching from the formula, stop its service and unlink it first.
  EOS
end
