using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace SuperDuper.Windows.Views;

public partial class ScanProgressView : UserControl
{
    public ScanProgressView() => InitializeComponent();

    private void OnCancelButtonIsEnabledChanged(
        object sender,
        DependencyPropertyChangedEventArgs eventArgs)
    {
        if (sender is Button button
            && eventArgs.NewValue is false
            && (button.IsKeyboardFocused || ReferenceEquals(Keyboard.FocusedElement, button)))
        {
            ScanProgressStatusHeading.Focus();
        }
    }
}
