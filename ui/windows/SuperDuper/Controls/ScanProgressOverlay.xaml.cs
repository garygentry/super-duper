using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using SuperDuper.ViewModels;

namespace SuperDuper.Controls;

public sealed partial class ScanProgressOverlay : UserControl
{
    private ScanDialogViewModel? _vm;

    // x:Bind source properties (delegated to ViewModel)
    public string ScanPhaseLabel => _vm?.ScanPhaseLabel ?? "";
    public bool ScanProgressIndeterminate => _vm?.ScanProgressIndeterminate ?? true;
    public double ScanProgressMax => _vm?.ScanProgressMax ?? 1;
    public double ScanProgressValue => _vm?.ScanProgressValue ?? 0;
    public string ScanCountLabel => _vm?.ScanCountLabel ?? "";
    public string CurrentFilePath => _vm?.CurrentFilePath ?? "";

    public string SpeedLabel { get; private set; } = "";

    public ScanProgressOverlay()
    {
        this.InitializeComponent();
    }

    public void Bind(ScanDialogViewModel vm)
    {
        _vm = vm;
        _vm.PropertyChanged += (_, _) => Bindings.Update();
    }

    private void CancelButton_Click(object sender, RoutedEventArgs e)
    {
        _vm?.CancelScanCommand.Execute(null);
    }
}
