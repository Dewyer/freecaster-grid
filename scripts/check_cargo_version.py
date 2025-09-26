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
    if doc["package"]["version"] != new_version:
        print(
            f"Version in Cargo.toml ('package'.'version') does not match computed version ({new_version}).",
            file=sys.stderr,
        )
        print("true")
    else:
        print(
            f"Version in Cargo.toml matches computed version: {new_version}",
            file=sys.stderr,
        )
        print("false")


if __name__ == "__main__":
    main(sys.argv[1])
