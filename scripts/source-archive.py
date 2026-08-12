#!/usr/bin/env python3

import gzip
import os
import subprocess
import sys
import tarfile


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} <output.tar.gz>")

    output = sys.argv[1]
    paths = subprocess.check_output(
        ["git", "ls-files", "--recurse-submodules", "-z"]
    ).split(b"\0")

    with open(output, "wb") as archive_file:
        with gzip.GzipFile(filename="", fileobj=archive_file, mode="wb", mtime=0) as gzip_file:
            with tarfile.open(fileobj=gzip_file, mode="w") as archive:
                for raw_path in paths:
                    if not raw_path:
                        continue
                    path = os.fsdecode(raw_path)
                    if not os.path.lexists(path):
                        continue
                    archive.add(
                        path,
                        arcname=f"vide-source/{path}",
                        recursive=False,
                        filter=normalize,
                    )


def normalize(info: tarfile.TarInfo) -> tarfile.TarInfo:
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    return info


if __name__ == "__main__":
    main()
