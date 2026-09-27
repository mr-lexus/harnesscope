class Harnesscope < Formula
  desc "Local cross-platform telemetry utility for AI coding agents"
  homepage "https://github.com/mr-lexus/harnesscope"
  version "0.1.0"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.1.0/harnesscope-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    else
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.1.0/harnesscope-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    url "https://github.com/mr-lexus/harnesscope/releases/download/v0.1.0/harnesscope-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  end

  def install
    bin.install "harnesscope"
  end

  service do
    run [opt_bin/"harnesscope", "serve"]
    keep_alive true
    log_path var/"log/harnesscope.log"
    error_log_path var/"log/harnesscope.error.log"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/harnesscope --version")
    assert_match "Harnesscope", shell_output("#{bin}/harnesscope --help")
  end
end
