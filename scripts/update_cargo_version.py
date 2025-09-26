# /// script
# requires-python = ">=3.12"
# dependencies = ["tomlkit==0.13.3"]
# ///

from pathlib import Path
import sys
import tomlkit  # type: ignore


def main(new_version: str) -> None:
    cargo_toml_path = Path("Cargo.toml")
    doc = tomlkit.parse(cargo_toml_path.read_text())
    doc["package"]["version"] = new_version
    cargo_toml_path.write_text(tomlkit.dumps(doc))


if __name__ == "__main__":
    main(sys.argv[1])
