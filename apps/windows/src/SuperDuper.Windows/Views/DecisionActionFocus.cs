using CommunityToolkit.Mvvm.Input;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;

namespace SuperDuper.Windows.Views;

internal static class DecisionActionFocus
{
    // Click is raised before Button executes its command. Move off the button before
    // the async command disables it, keeping a stable anchor across member reloads.
    internal static async Task PreserveAsync(Button button, FrameworkElement anchor, Func<bool> isCurrent)
    {
        if (!button.IsKeyboardFocusWithin || button.Command is not IAsyncRelayCommand command
            || !command.CanExecute(button.CommandParameter) || !anchor.Focus()) return;

        await button.Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.Background);
        if (command.ExecutionTask is { } execution)
        {
            try { await execution; }
            catch { return; } // The command owns error reporting; do not rethrow its exception twice.
        }
        for (var attempt = 0; attempt < 8; attempt++)
        {
            var finished = await button.Dispatcher.InvokeAsync(() =>
            {
                // Never pull focus back after navigation or a change of selected copy.
                if (!anchor.IsKeyboardFocused || !isCurrent()) return true;
                return button.IsVisible && button.IsEnabled && button.Focus();
            }, DispatcherPriority.ContextIdle);
            if (finished) return;
            await button.Dispatcher.InvokeAsync(static () => { }, DispatcherPriority.Background);
        }
    }
}
