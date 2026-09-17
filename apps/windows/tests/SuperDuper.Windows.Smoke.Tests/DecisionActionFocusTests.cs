using CommunityToolkit.Mvvm.Input;
using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Threading;
using SuperDuper.Windows.Views;

namespace SuperDuper.Windows.Smoke.Tests;

internal static class DecisionActionFocusTests
{
    internal static void Verify()
    {
        var previousContext = SynchronizationContext.Current;
        // InvokeClick is called directly by the fixture. Match the synchronization
        // context WPF installs when it dispatches a real keyboard/button event.
        SynchronizationContext.SetSynchronizationContext(new DispatcherSynchronizationContext());
        var anchor = new TextBlock { Text = "Selected set", Focusable = true };
        KeyboardNavigation.SetIsTabStop(anchor, false);
        var action = new ClickableButton { Content = "Keep copy" };
        var nextAction = new Button { Content = "Mark copy for removal" };
        var panel = new StackPanel();
        panel.Children.Add(anchor);
        panel.Children.Add(action);
        panel.Children.Add(nextAction);
        var window = new Window { Content = panel, Width = 450, Height = 220 };
        window.Show();
        try
        {
            foreach (var scenario in new[] { "complete", "navigate", "selection-changed", "synchronous" })
            {
                var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
                var executions = 0;
                action.Command = new AsyncRelayCommand(() =>
                {
                    executions++;
                    return scenario == "synchronous" ? Task.CompletedTask : release.Task;
                });
                var current = true;
                Task? restoration = null;
                RoutedEventHandler handler = (_, _) =>
                    restoration = DecisionActionFocus.PreserveAsync(action, anchor, () => current);
                action.Click += handler;
                Assert.IsTrue(action.Focus());
                action.InvokeClick();
                DrainDispatcher();
                if (scenario != "synchronous")
                {
                    Assert.IsFalse(action.IsEnabled, "The real async command must disable the originating button.");
                    Assert.IsTrue(anchor.IsKeyboardFocused, "Focus needs a stable anchor while the command runs.");
                }
                if (scenario == "navigate") Assert.IsTrue(nextAction.Focus());
                if (scenario == "selection-changed") current = false;
                release.SetResult();
                var timeout = Stopwatch.StartNew();
                while (restoration is { IsCompleted: false } && timeout.Elapsed < TimeSpan.FromSeconds(5))
                    DrainDispatcher();
                Assert.IsNotNull(restoration);
                Assert.IsTrue(restoration.IsCompleted, "Decision focus restoration did not finish within five seconds.");
                restoration!.GetAwaiter().GetResult();
                Assert.AreEqual(1, executions, "The focus hook must leave command execution to the button.");
                if (scenario is "complete" or "synchronous")
                {
                    Assert.IsTrue(action.IsKeyboardFocused,
                        $"{scenario}: enabled={action.IsEnabled}, visible={action.IsVisible}, anchorFocused={anchor.IsKeyboardFocused}.");
                    Assert.IsTrue(action.MoveFocus(new TraversalRequest(FocusNavigationDirection.Next)));
                    Assert.IsTrue(nextAction.IsKeyboardFocused, "Tab must continue at the neighboring decision action.");
                }
                else if (scenario == "navigate") Assert.IsTrue(nextAction.IsKeyboardFocused);
                else Assert.IsTrue(anchor.IsKeyboardFocused);
                action.Click -= handler;
            }
        }
        finally
        {
            window.Close();
            SynchronizationContext.SetSynchronizationContext(previousContext);
        }
    }

    private static void DrainDispatcher()
    {
        var frame = new DispatcherFrame();
        Dispatcher.CurrentDispatcher.BeginInvoke(() => frame.Continue = false, DispatcherPriority.ApplicationIdle);
        Dispatcher.PushFrame(frame);
    }

    private sealed class ClickableButton : Button
    {
        internal void InvokeClick() => OnClick();
    }
}
