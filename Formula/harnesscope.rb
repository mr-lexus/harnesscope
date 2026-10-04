class Harnesscope < Formula
  desc "Local cross-platform telemetry utility for AI coding agents"
  homepage "https://github.com/mr-lexus/harnesscope"
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.3/harnesscope-v0.2.3-aarch64-apple-darwin.tar.gz"
      sha256 "781fa55db918d0cc9db159e58e866f22dd196ac2ed8316fffdbe9421209b66e8"
    else
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.3/harnesscope-v0.2.3-x86_64-apple-darwin.tar.gz"
      sha256 "6d92439203fca447badc39f03d6f9caa3fac2b97d575b29b01fcff93737b4016"
    end
  else
    url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.3/harnesscope-v0.2.3-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "04a549797f439054d7c3767ff9ff0f8c308a314d2671f0f9fa54519a4e2de07d"
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
