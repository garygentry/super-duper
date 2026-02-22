using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Models;

namespace SuperDuper.Controls;

public sealed partial class DensityBadge : UserControl
{
    public static readonly DependencyProperty CountProperty =
        DependencyProperty.Register(nameof(Count), typeof(int), typeof(DensityBadge),
            new PropertyMetadata(0, OnPropsChanged));

    public static readonly DependencyProperty LevelProperty =
        DependencyProperty.Register(nameof(Level), typeof(DensityLevel), typeof(DensityBadge),
            new PropertyMetadata(DensityLevel.Low, OnPropsChanged));

    public int Count
    {
        get => (int)GetValue(CountProperty);
        set => SetValue(CountProperty, value);
    }

    public DensityLevel Level
    {
        get => (DensityLevel)GetValue(LevelProperty);
        set => SetValue(LevelProperty, value);
    }

    public DensityBadge()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
    }

    private static void OnPropsChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is DensityBadge badge) badge.Update();
    }

    private void Update()
    {
        BadgeText.Text = Count.ToString();

        var bgKey = Level switch
        {
            DensityLevel.High => "DensityHighBg",
            DensityLevel.Medium => "DensityMedBg",
            _ => "DensityLowBg"
        };

        var fgKey = Level switch
        {
            DensityLevel.High => "DensityHighFg",
            DensityLevel.Medium => "DensityMedFg",
            _ => "DensityLowFg"
        };

        if (Resources.TryGetValue(bgKey, out var bg) && bg is SolidColorBrush bgBrush)
            BadgeBorder.Background = bgBrush;
        else if (Application.Current.Resources.TryGetValue(bgKey, out bg) && bg is SolidColorBrush bgBrush2)
            BadgeBorder.Background = bgBrush2;

        if (Resources.TryGetValue(fgKey, out var fg) && fg is SolidColorBrush fgBrush)
            BadgeText.Foreground = fgBrush;
        else if (Application.Current.Resources.TryGetValue(fgKey, out fg) && fg is SolidColorBrush fgBrush2)
            BadgeText.Foreground = fgBrush2;

        var levelStr = Level switch
        {
            DensityLevel.High => "High",
            DensityLevel.Medium => "Medium",
            _ => "Low"
        };
        AutomationProperties.SetName(this, $"{Count} duplicates, {levelStr} density");
    }
}
