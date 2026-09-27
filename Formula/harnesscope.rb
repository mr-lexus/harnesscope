class Harnesscope < Formula
  desc "Local cross-platform telemetry utility for AI coding agents"
  homepage "https://github.com/mr-lexus/harnesscope"
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.2/harnesscope-v0.2.2-aarch64-apple-darwin.tar.gz"
      sha256 "0c9e43d741f7707174ab53d0ffaf02c8fd1fc46404e342b33e2de188a1cc8628"
    else
      url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.2/harnesscope-v0.2.2-x86_64-apple-darwin.tar.gz"
      sha256 "ca097bedfdad15dc166b6f07257d957fdae0203a6225675d558e77f2374d4a10"
    end
  else
    url "https://github.com/mr-lexus/harnesscope/releases/download/v0.2.2/harnesscope-v0.2.2-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "0a0440a8578ecefb7e62113ee2a9ddec87af8900cfa1349806eb61442cf31b01"
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
