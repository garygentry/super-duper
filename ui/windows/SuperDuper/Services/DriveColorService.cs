using Microsoft.UI;
using Windows.UI;

namespace SuperDuper.Services;

/// <summary>
/// Maps drive letters to one of 8 stable palette colors.
/// Color assignment is deterministic (hash of drive letter) so it survives restarts.
/// </summary>
public class DriveColorService
{
    // 8 visually distinct colors suitable for both light and dark themes
    private static readonly Color[] Palette =
    [
        Color.FromArgb(255, 0, 120, 212),   // Drive0: Windows Blue
        Color.FromArgb(255, 0, 153, 92),    // Drive1: Teal
        Color.FromArgb(255, 137, 68, 171),  // Drive2: Purple
        Color.FromArgb(255, 202, 80, 16),   // Drive3: Orange
        Color.FromArgb(255, 232, 17, 35),   // Drive4: Red
        Color.FromArgb(255, 0, 178, 148),   // Drive5: Cyan-green
        Color.FromArgb(255, 195, 0, 82),    // Drive6: Crimson
        Color.FromArgb(255, 77, 77, 77),    // Drive7: Gray (for UNC/unknown)
    ];

    public Color GetColor(string driveLetter)
    {
        if (string.IsNullOrEmpty(driveLetter))
            return Palette[7]; // Gray for unknown/network

        var index = Math.Abs(driveLetter.ToUpperInvariant().GetHashCode()) % (Palette.Length - 1);
        return Palette[index];
    }

    public Color GetColorByIndex(int index) => Palette[index % Palette.Length];
}
