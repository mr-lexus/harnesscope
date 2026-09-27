class Harnesscope < Formula
  desc "Local cross-platform telemetry utility for AI coding agents"
  homepage "https://github.com/mr-lexus/harnesscope"
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.1/harnesscope-v0.2.1-aarch64-apple-darwin.tar.gz"
      sha256 "ad0d5656f5f17881610dd6627f5f225b2f5b39b0cd16020ebc40983efe69dc72"
    else
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.1/harnesscope-v0.2.1-x86_64-apple-darwin.tar.gz"
      sha256 "c9f75c16797db6f95a47ed2593bef76ec207afdb11798d6471cb5e9fe51ac4d5"
    end
  else
    url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.1/harnesscope-v0.2.1-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "9f8d753953e73cbcc15e8bec515c4a3808fae61ee1d29c0924d7e57eb3424ca3"
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
