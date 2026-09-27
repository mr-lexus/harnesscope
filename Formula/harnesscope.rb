class Harnesscope < Formula
  desc "Local cross-platform telemetry utility for AI coding agents"
  homepage "https://github.com/mr-lexus/harnesscope"
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.1.0/harnesscope-v0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "387ed35e49b042a0c4166cf44590d68e64cf0f6650740ddd98aee085e7910044"
    else
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.1.0/harnesscope-v0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "a228c5b7769c7e096ac0b0929ff85f985f3bd545922ff9202162a0412864d4f3"
    end
  else
    url "https://github.com/mr-lexus/harnesscope/releases/download/v0.1.0/harnesscope-v0.1.0-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "4e3f0423bf9779a651ead22d84a153a7322f956b6e3c6ab407b0acd45dd6f8ee"
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
