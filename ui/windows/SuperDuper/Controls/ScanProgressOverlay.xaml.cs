using System.ComponentModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.Services;

namespace SuperDuper.Controls;

public sealed partial class ScanProgressOverlay : UserControl, INotifyPropertyChanged
{
    private ScanService? _service;

    // Binding source properties (delegated to ScanService)
    public string ScanPhaseLabel => _service?.ScanPhaseLabel ?? "";
    public bool ScanProgressIndeterminate => _service?.ScanProgressIndeterminate ?? true;
    public double ScanProgressMax => _service?.ScanProgressMax ?? 1;
    public double ScanProgressValue => _service?.ScanProgressValue ?? 0;
    public string ScanCountLabel => _service?.ScanCountLabel ?? "";
    public string CurrentFilePath => _service?.CurrentFilePath ?? "";

    public string SpeedLabel { get; private set; } = "";

    public event PropertyChangedEventHandler? PropertyChanged;

    public ScanProgressOverlay()
    {
        this.InitializeComponent();
        XamlHelper.ConnectNamedElements(this, this);
        this.DataContext = this;

        // Wire events (XAML compiler pass 2 doesn't generate IComponentConnector)
        CancelButton.Click += CancelButton_Click;
    }

    public void Bind(ScanService service)
    {
        _service = service;
        _service.PropertyChanged += Service_PropertyChanged;
    }

    private void Service_PropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName is null)
        {
            // Bulk refresh — forward all properties
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ScanPhaseLabel)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ScanProgressIndeterminate)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ScanProgressMax)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ScanProgressValue)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(ScanCountLabel)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(CurrentFilePath)));
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(SpeedLabel)));
        }
        else
        {
            // Forward the specific property change
            PropertyChanged?.Invoke(this, e);
        }
    }

    private void CancelButton_Click(object sender, RoutedEventArgs e)
    {
        _service?.CancelScan();
    }
}
