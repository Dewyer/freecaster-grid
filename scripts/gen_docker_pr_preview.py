# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///

import sys


def main() -> None:
    input_data = sys.stdin.read()
    tags = input_data.splitlines()
    if len(tags) == 0:
        print("No tags found")
        sys.exit(0)
    print(f"Docker image{len(tags) > 1 and 's' or ''} for this PR are available:")
    print("`" * 3)
    for tag in tags:
        print(tag)
    print("`" * 3)
    print("To try locally, run:")
    print("`" * 3)
    print(f"docker pull {tags[0]}")
    print("`" * 3)


if __name__ == "__main__":
    main()
