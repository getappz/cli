#!/usr/bin/env python3
"""Split Microsoft Rust guidelines (all.txt) into smaller files by section.
Based on the 40tude PowerShell script for cross-platform use."""

import re
import sys
from pathlib import Path

def main():
    script_dir = Path(__file__).resolve().parent
    input_path = script_dir / "rust-guidelines.txt"
    if not input_path.exists():
        print(f"Error: {input_path} not found", file=sys.stderr)
        sys.exit(1)

    content = input_path.read_text(encoding="utf-8")
    lines = re.split(r"\r?\n", content)

    # Find separator lines
    sep_indices = [i for i, line in enumerate(lines) if line.strip() == "---"]
    if len(sep_indices) < 2:
        print("No section separators found.")
        sys.exit(0)

    file_counter = 1
    for k in range(len(sep_indices) - 1):
        start_idx = sep_indices[k] + 1
        end_idx = sep_indices[k + 1] - 1
        if end_idx < start_idx:
            continue

        section_lines = lines[start_idx : end_idx + 1]

        # Find H1 headings
        h1_indices = [
            j for j, line in enumerate(section_lines)
            if re.match(r"^[ \t]*#\s+", line)
        ]
        if not h1_indices:
            continue

        rel_start = h1_indices[0]
        rel_end = len(section_lines) - 1
        extract_lines = section_lines[rel_start : rel_end + 1]
        extract_lines = [ln for ln in extract_lines if ln.strip() != "---"]

        # Title from first line
        title_line = re.sub(r"^[ \t]*#\s+", "", extract_lines[0]).strip()
        filename_base = title_line.lower().strip()
        filename_base = re.sub(r"[\s/]+", "_", filename_base)
        filename_base = re.sub(r"[^a-z0-9_]+", "_", filename_base)
        filename_base = filename_base.strip("_")
        if not filename_base:
            filename_base = "section"

        index = f"{file_counter:02d}"
        out_name = f"{index}_{filename_base}.md"
        out_path = script_dir / out_name
        out_path.write_text("\n".join(extract_lines) + "\n", encoding="utf-8")
        print(f"Wrote: {out_name}")
        file_counter += 1

    print(f"Done. Generated {file_counter - 1} file(s).")


if __name__ == "__main__":
    main()
