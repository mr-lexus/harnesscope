#!/usr/bin/env python3
"""Generate formula and binary cask from one verified release checksum manifest."""
import argparse
from pathlib import Path
import re


def generate(version: str, manifest: str, destination: Path) -> None:
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?", version):
        raise ValueError("Expected a release tag such as v0.2.3")
    hashes = {}
    for line in manifest.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-fA-F]{64})\s+\*?(\S+)", line)
        if not match or match[2] in hashes:
            raise ValueError("Malformed or duplicate release checksum")
        hashes[match[2]] = match[1].lower()
    targets = ("aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu")
    names = [f"harnesscope-{version}-{target}.tar.gz" for target in targets]
    if any(name not in hashes for name in names):
        raise ValueError("Missing release archive checksum")
    arm, intel, linux = [hashes[name] for name in names]
    root = f"https://github.com/mr-lexus/harnesscope/releases/download/{version}"
    formula = f'''class Harnesscope < Formula
  desc "Local cross-platform telemetry utility for AI coding agents"
  homepage "https://github.com/mr-lexus/harnesscope"
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "{root}/{names[0]}"
      sha256 "{arm}"
    else
      url "{root}/{names[1]}"
      sha256 "{intel}"
    end
  else
    url "{root}/{names[2]}"
    sha256 "{linux}"
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
    assert_match version.to_s, shell_output("#{{bin}}/harnesscope --version")
    assert_match "Harnesscope", shell_output("#{{bin}}/harnesscope --help")
  end
end
'''
    cask = f'''cask "harnesscope" do
  arch arm: "aarch64", intel: "x86_64"

  version "{version[1:]}"
  sha256 arm:   "{arm}",
         intel: "{intel}"

  url "https://github.com/mr-lexus/harnesscope/releases/download/v#{{version}}/harnesscope-v#{{version}}-#{{arch}}-apple-darwin.tar.gz"
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
'''
    for folder, text in (("Formula", formula), ("Casks", cask)):
        path = destination / folder / "harnesscope.rb"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--checksums", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=Path("."))
    args = parser.parse_args()
    generate(args.version, args.checksums.read_text(encoding="utf-8"), args.output)
