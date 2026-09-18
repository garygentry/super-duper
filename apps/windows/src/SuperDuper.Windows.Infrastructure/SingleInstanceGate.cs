using System.Security.Cryptography;
using System.Text;
using Windows.Win32;

namespace SuperDuper.Windows.Infrastructure;

/// <summary>
/// One app instance per state directory. The first instance creates a named event and listens on
/// it; a later instance signals that event so the first can bring its window forward, then exits.
/// The worker's database lock remains the backstop for anything this does not cover.
/// </summary>
public sealed class SingleInstanceGate : IDisposable
{
    private readonly EventWaitHandle _activation;
    private readonly ManualResetEvent _stopping = new(false);
    private readonly Thread _listener;

    private SingleInstanceGate(EventWaitHandle activation, Action onActivationRequested)
    {
        _activation = activation;
        _listener = new Thread(() =>
        {
            while (WaitHandle.WaitAny([_activation, _stopping]) == 0)
            {
                onActivationRequested();
            }
        })
        {
            IsBackground = true,
            Name = "SuperDuper single-instance activation",
        };
        _listener.Start();
    }

    /// <summary>
    /// Returns the gate when this is the first instance for <paramref name="stateDirectory"/>.
    /// Otherwise asks the running instance to activate and returns null.
    /// </summary>
    public static SingleInstanceGate? TryAcquire(string stateDirectory, Action onActivationRequested)
    {
        var activation = new EventWaitHandle(false, EventResetMode.AutoReset, EventName(stateDirectory), out var createdNew);
        if (createdNew)
        {
            return new SingleInstanceGate(activation, onActivationRequested);
        }

        // Let the running instance take the foreground; this process is the one the user just started.
        PInvoke.AllowSetForegroundWindow(PInvoke.ASFW_ANY);
        activation.Set();
        activation.Dispose();
        return null;
    }

    internal static string EventName(string stateDirectory)
    {
        var normalized = Path.TrimEndingDirectorySeparator(Path.GetFullPath(stateDirectory)).ToUpperInvariant();
        var hash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(normalized)))[..32];
        return $@"Local\SuperDuper-{hash}-activate";
    }

    public void Dispose()
    {
        _stopping.Set();
        _listener.Join(TimeSpan.FromSeconds(2));
        _activation.Dispose();
        _stopping.Dispose();
    }
}
