using System.Diagnostics;

namespace SuperDuper;

/// <summary>
/// Extension methods for fire-and-forget task execution with fault logging.
/// </summary>
internal static class TaskExtensions
{
    /// <summary>
    /// Observes the task for faults, logging any exceptions to Debug output.
    /// Replaces bare <c>_ = SomeAsync()</c> discards to prevent silent swallowing.
    /// </summary>
    public static void FireAndForget(this Task task, string caller)
    {
        task.ContinueWith(
            t => Debug.WriteLine($"[FireAndForget] {caller} faulted: {t.Exception!.Flatten()}"),
            TaskContinuationOptions.OnlyOnFaulted);
    }
}
