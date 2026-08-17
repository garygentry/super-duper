"""Build the Super Duper Windows icon set from a transparent source PNG."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image, ImageFilter


ICO_SIZES = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)
PNG_SIZES = (*ICO_SIZES, 512, 1024)
MASTER_SIZE = 1024
SUBJECT_FILL = 0.88
MIN_VISIBLE_ALPHA = 8


def prepare_master(source_path: Path) -> Image.Image:
    source = Image.open(source_path).convert("RGBA")

    # Remove generator edge noise that is below visible opacity, then crop using
    # the same threshold so isolated near-transparent pixels cannot skew centering.
    red, green, blue, alpha = source.split()
    alpha = alpha.point(lambda value: 0 if value < MIN_VISIBLE_ALPHA else value)
    source = Image.merge("RGBA", (red, green, blue, alpha))
    bounds = alpha.getbbox()
    if bounds is None:
        raise ValueError(f"Source has no visible pixels: {source_path}")

    subject = source.crop(bounds)
    target_extent = round(MASTER_SIZE * SUBJECT_FILL)
    scale = min(target_extent / subject.width, target_extent / subject.height)
    fitted_size = (
        max(1, round(subject.width * scale)),
        max(1, round(subject.height * scale)),
    )
    subject = subject.resize(fitted_size, Image.Resampling.LANCZOS)

    master = Image.new("RGBA", (MASTER_SIZE, MASTER_SIZE), (0, 0, 0, 0))
    position = (
        (MASTER_SIZE - subject.width) // 2,
        (MASTER_SIZE - subject.height) // 2,
    )
    master.alpha_composite(subject, position)
    return master


def render_size(master: Image.Image, size: int) -> Image.Image:
    rendered = master.resize((size, size), Image.Resampling.LANCZOS)
    if size <= 48:
        # A restrained, size-specific sharpen keeps the file edges and central
        # sparkle distinct at Windows list/detail-view sizes.
        radius = 0.35 if size <= 24 else 0.5
        rendered = rendered.filter(
            ImageFilter.UnsharpMask(radius=radius, percent=115, threshold=3)
        )
    return rendered


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output_directory", type=Path)
    args = parser.parse_args()

    args.output_directory.mkdir(parents=True, exist_ok=True)
    master = prepare_master(args.source)
    frames = {size: render_size(master, size) for size in PNG_SIZES}

    for size, frame in frames.items():
        frame.save(
            args.output_directory / f"SuperDuper-{size}.png",
            format="PNG",
            optimize=True,
        )

    icon_frames = [frames[size] for size in ICO_SIZES]
    master.save(
        args.output_directory / "SuperDuper.ico",
        format="ICO",
        sizes=[(size, size) for size in ICO_SIZES],
        append_images=icon_frames,
        bitmap_format="png",
    )


if __name__ == "__main__":
    main()
