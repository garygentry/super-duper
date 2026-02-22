using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using SuperDuper.Models;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using Windows.Foundation;

namespace SuperDuper.Controls;

/// <summary>
/// Squarified treemap of directory storage colored green→red by dupe density.
/// Runs layout on Task.Run; only UI updates happen on the dispatcher.
/// </summary>
public sealed partial class StorageTreemap : UserControl
{
    public static readonly DependencyProperty ItemsSourceProperty =
        DependencyProperty.Register(nameof(ItemsSource), typeof(IList<TreemapNode>),
            typeof(StorageTreemap), new PropertyMetadata(null, OnItemsSourceChanged));

    public IList<TreemapNode>? ItemsSource
    {
        get => (IList<TreemapNode>?)GetValue(ItemsSourceProperty);
        set => SetValue(ItemsSourceProperty, value);
    }

    public event EventHandler<TreemapNode>? NodeClicked;

    private record TreemapRect(TreemapNode Node, Rect Bounds);
    private List<TreemapRect> _rects = new();

    public StorageTreemap()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        TreemapCanvas.SizeChanged += TreemapCanvas_SizeChanged;
    }

    private bool _renderPending;

    private static void OnItemsSourceChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is not StorageTreemap tm) return;

        if (e.OldValue is INotifyCollectionChanged oldCollection)
            oldCollection.CollectionChanged -= tm.OnItemsCollectionChanged;

        if (e.NewValue is INotifyCollectionChanged newCollection)
            newCollection.CollectionChanged += tm.OnItemsCollectionChanged;

        _ = tm.RenderAsync();
    }

    private void OnItemsCollectionChanged(object? sender, NotifyCollectionChangedEventArgs e)
    {
        if (!_renderPending)
        {
            _renderPending = true;
            DispatcherQueue.TryEnqueue(() =>
            {
                _renderPending = false;
                _ = RenderAsync();
            });
        }
    }

    private void TreemapCanvas_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        _ = RenderAsync();
    }

    private async Task RenderAsync()
    {
        var items = ItemsSource;
        if (items == null || items.Count == 0)
        {
            TreemapCanvas.Children.Clear();
            EmptyText.Visibility = Visibility.Visible;
            return;
        }

        EmptyText.Visibility = Visibility.Collapsed;

        var width = TreemapCanvas.ActualWidth;
        var height = TreemapCanvas.ActualHeight;
        if (width < 1 || height < 1) return;

        // Compute layout off UI thread
        var rects = await Task.Run(() => Squarify(items, new Rect(0, 0, width, height)));
        _rects = rects;

        // Render on UI thread
        TreemapCanvas.Children.Clear();
        foreach (var r in rects)
        {
            var cell = CreateCell(r);
            Canvas.SetLeft(cell, r.Bounds.X);
            Canvas.SetTop(cell, r.Bounds.Y);
            cell.Width = r.Bounds.Width;
            cell.Height = r.Bounds.Height;
            TreemapCanvas.Children.Add(cell);
        }
    }

    private Border CreateCell(TreemapRect r)
    {
        var density = r.Node.DupeDensity;
        var bg = InterpolateDensityColor(density);

        var cell = new Border
        {
            Background = new SolidColorBrush(bg),
            BorderBrush = new SolidColorBrush(Windows.UI.Color.FromArgb(40, 0, 0, 0)),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(2),
        };

        // Show label if cell is large enough
        if (r.Bounds.Width > 80 && r.Bounds.Height > 60)
        {
            var label = new TextBlock
            {
                Text = r.Node.DisplayName,
                FontSize = 11,
                TextTrimming = TextTrimming.CharacterEllipsis,
                TextWrapping = TextWrapping.NoWrap,
                MaxWidth = r.Bounds.Width - 8,
                Margin = new Thickness(4, 4, 4, 0),
                Foreground = new SolidColorBrush(Colors.White),
            };
            cell.Child = label;
        }

        ToolTipService.SetToolTip(cell,
            $"{r.Node.DisplayName}\n{FileSizeLabel(r.Node.TotalBytes)}\n{(int)(r.Node.DupeDensity * 100)}% duplicates");

        cell.Tapped += (_, _) => NodeClicked?.Invoke(this, r.Node);
        return cell;
    }

    private static string FileSizeLabel(long bytes)
    {
        if (bytes >= 1_073_741_824) return $"{bytes / 1_073_741_824.0:F1} GB";
        if (bytes >= 1_048_576) return $"{bytes / 1_048_576.0:F1} MB";
        if (bytes >= 1024) return $"{bytes / 1024.0:F1} KB";
        return $"{bytes} B";
    }

    private static Windows.UI.Color InterpolateDensityColor(double density)
    {
        // 0.0 = green (#2D7D46), 1.0 = red (#C42B1C)
        var r = (byte)(0x2D + (0xC4 - 0x2D) * density);
        var g = (byte)(0x7D + (0x2B - 0x7D) * density);
        var b = (byte)(0x46 + (0x1C - 0x46) * density);
        return Windows.UI.Color.FromArgb(200, r, g, b);
    }

    // Squarified treemap layout algorithm
    private static List<TreemapRect> Squarify(IList<TreemapNode> nodes, Rect bounds)
    {
        var result = new List<TreemapRect>();
        var totalBytes = nodes.Sum(n => (double)n.TotalBytes);
        if (totalBytes <= 0) return result;

        var sorted = nodes.OrderByDescending(n => n.TotalBytes).ToList();
        SquarifyRow(sorted, 0, bounds, totalBytes, result);
        return result;
    }

    private static void SquarifyRow(
        List<TreemapNode> nodes, int start, Rect bounds,
        double totalBytes, List<TreemapRect> result)
    {
        if (start >= nodes.Count || bounds.Width < 1 || bounds.Height < 1) return;

        bool horizontal = bounds.Width >= bounds.Height;
        double shortSide = horizontal ? bounds.Height : bounds.Width;
        double longSide = horizontal ? bounds.Width : bounds.Height;

        var row = new List<TreemapNode>();
        double rowBytes = 0;
        double bestAspect = double.MaxValue;
        int end = start;

        while (end < nodes.Count)
        {
            var candidate = nodes[end];
            var candidateBytes = (double)candidate.TotalBytes;
            var newRowBytes = rowBytes + candidateBytes;
            var rowFraction = newRowBytes / totalBytes;
            var rowLength = rowFraction * longSide;

            // Compute worst aspect ratio in this row
            double worst = 0;
            double tempBytes = 0;
            foreach (var n in row)
            {
                tempBytes += n.TotalBytes;
                var cellLength = (n.TotalBytes / newRowBytes) * rowLength;
                var aspect = shortSide / cellLength;
                if (aspect < 1) aspect = 1 / aspect;
                if (aspect > worst) worst = aspect;
            }
            var newCellLength = (candidateBytes / newRowBytes) * rowLength;
            var newAspect = shortSide / newCellLength;
            if (newAspect < 1) newAspect = 1 / newAspect;
            if (newAspect > worst) worst = newAspect;

            if (row.Count > 0 && worst > bestAspect) break;

            row.Add(candidate);
            rowBytes = newRowBytes;
            bestAspect = worst;
            end++;
        }

        // Lay out the row
        double rowFrac = rowBytes / totalBytes;
        double rowLen = rowFrac * longSide;
        double pos = 0;
        foreach (var n in row)
        {
            var frac = n.TotalBytes / rowBytes;
            Rect cellBounds;
            if (horizontal)
                cellBounds = new Rect(bounds.X + pos, bounds.Y, frac * rowLen, shortSide);
            else
                cellBounds = new Rect(bounds.X, bounds.Y + pos, shortSide, frac * rowLen);

            // Enforce minimum cell size
            if (cellBounds.Width >= 5 && cellBounds.Height >= 5)
                result.Add(new TreemapRect(n, cellBounds));

            pos += frac * rowLen;
        }

        // Recurse on remaining space
        Rect remaining;
        if (horizontal)
            remaining = new Rect(bounds.X + rowLen, bounds.Y, bounds.Width - rowLen, bounds.Height);
        else
            remaining = new Rect(bounds.X, bounds.Y + rowLen, bounds.Width, bounds.Height - rowLen);

        SquarifyRow(nodes, end, remaining, totalBytes - rowBytes, result);
    }
}
