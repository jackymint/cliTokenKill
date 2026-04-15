class Ctk < Formula
  desc "Reduce terminal output before it reaches AI assistant context"
  homepage "https://github.com/jackymint/cliTokenKill"
  url "https://github.com/jackymint/cliTokenKill/archive/refs/tags/v0.30.0.tar.gz"
  sha256 "PLACEHOLDER"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "ctk", shell_output("#{bin}/ctk --help")
  end
end
