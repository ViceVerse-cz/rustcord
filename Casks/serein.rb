cask "serein" do
  version "1.0.0-nightly.5.1"
  sha256 "5a8a00447ec8d9596182119d2a4a569f9ac75efb615e623f75f295287cfc1a1d"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
